use std::{future::Future, sync::Arc, time::Duration};

use anyhow::{Error, Result};
use eframe::egui;
use egui_async::{Bind, EguiAsyncPlugin};
use tracing::{error, info};

use crate::config::{self, AppConfig, UserConfig};
use crate::db::{Credentials, Db, LoginSession};
use crate::theme::Theme;

enum Screen {
    Login,
    Dashboard,
}

enum StatusKind {
    Info,
    Success,
    Error,
}

struct Status {
    kind: StatusKind,
    message: String,
}

enum AppAction {
    LoginSuccess {
        session: LoginSession,
        remember: bool,
    },
    SessionUpdated {
        session: LoginSession,
        message: String,
    },
    AccountCreated,
}

pub struct LauncherApp {
    db: Arc<Db>,
    app_config: AppConfig,
    config: UserConfig,
    screen: Screen,
    status: Status,
    creds: Credentials,
    remember: bool,
    amount: String,
    selected_char: Option<usize>,
    current_session: Option<LoginSession>,
    action_bind: Bind<AppAction, Error>,
}

impl LauncherApp {
    pub fn new(app_config: AppConfig, db: Arc<Db>) -> Self {
        let config: UserConfig =
            config::read_json("config.json").unwrap_or_default();
        Self {
            db,
            app_config,
            screen: Screen::Login,
            status: Status {
                kind: StatusKind::Info,
                message: "Ready".to_string(),
            },
            creds: Credentials {
                username: config.username.clone(),
                password: config.password.clone(),
            },
            remember: config.remember,
            config,
            amount: String::new(),
            selected_char: None,
            current_session: None,
            action_bind: Bind::new(false),
        }
    }

    fn process_async(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.action_bind.take() {
            match result {
                Ok(action) => self.apply_action(action),
                Err(err) => self.status = Status::error(err.to_string()),
            }
            ctx.request_repaint();
        }
    }

    fn apply_action(&mut self, action: AppAction) {
        match action {
            AppAction::LoginSuccess {
                session,
                remember,
            } => {
                if remember {
                    self.config.username = self.creds.username.clone();
                    self.config.password = self.creds.password.clone();
                    self.config.remember = true;
                    let _ = config::write_json("config.json", &self.config);
                }
                self.current_session = Some(session);
                self.screen = Screen::Dashboard;
                self.status = Status::success("Login successful");
                self.selected_char = None;
            }
            AppAction::SessionUpdated { session, message } => {
                self.current_session = Some(session);
                self.status = Status::success(message);
            }
            AppAction::AccountCreated => {
                self.status = Status::success("Account created successfully!");
            }
        }
    }

    fn spawn_action<Fut>(&mut self, fut: Fut) -> Result<(), Status>
    where
        Fut: Future<Output = Result<AppAction, Error>> + Send + 'static,
    {
        if self.action_bind.is_pending() {
            return Err(Status::error("Operation in progress"));
        }
        self.action_bind.request(fut);
        Ok(())
    }

    fn credentials(&self) -> Credentials {
        self.creds.clone()
    }

    fn login(&mut self) -> Result<(), Status> {
        let creds = self.credentials();
        let db = self.db.clone();
        let remember = self.remember;
        tracing::info!("ui: login requested");
        self.spawn_action(async move {
            let session = db.perform_login(&creds.username, &creds.password).await?;
            Ok(AppAction::LoginSuccess {
                session,
                remember,
            })
        })
    }

    fn create_account(&mut self) -> Result<(), Status> {
        let creds = self.credentials();
        let db = self.db.clone();
        tracing::info!("ui: create account requested");
        self.spawn_action(async move {
            db.create_account(&creds.username, &creds.password).await?;
            Ok(AppAction::AccountCreated)
        })
    }

    fn refresh(&mut self) -> Result<(), Status> {
        let creds = self.credentials();
        let db = self.db.clone();
        tracing::debug!("ui: refresh requested");
        self.spawn_action(async move {
            let session = db.perform_login(&creds.username, &creds.password).await?;
            Ok(AppAction::SessionUpdated {
                session,
                message: "Data refreshed".to_string(),
            })
        })
    }

    fn send_gold(&mut self) -> Result<(), Status> {
        let amount = self.parse_amount()?;
        let Some(session) = &self.current_session else {
            return Err(Status::error("No session"));
        };
        let Some(idx) = self.selected_char else {
            return Err(Status::error("Select a character"));
        };
        let char_id = session.characters[idx].id;
        let db = self.db.clone();
        let creds = self.credentials();
        tracing::info!("ui: send gold requested");
        self.spawn_action(async move {
            db.send_gold(char_id, amount).await?;
            tokio::time::sleep(Duration::from_secs(1)).await;
            let session = db.perform_login(&creds.username, &creds.password).await?;
            Ok(AppAction::SessionUpdated {
                session,
                message: "Gold sent! Data refreshed".to_string(),
            })
        })
    }

    fn send_cera(&mut self) -> Result<(), Status> {
        let amount = self.parse_amount()?;
        let Some(session) = &self.current_session else {
            return Err(Status::error("No session"));
        };
        let uid = session.uid;
        let db = self.db.clone();
        let creds = self.credentials();
        tracing::info!("ui: send cera requested");
        self.spawn_action(async move {
            db.send_cera(uid, amount).await?;
            tokio::time::sleep(Duration::from_secs(1)).await;
            let session = db.perform_login(&creds.username, &creds.password).await?;
            Ok(AppAction::SessionUpdated {
                session,
                message: "Cera sent! Data refreshed".to_string(),
            })
        })
    }

    fn parse_amount(&self) -> Result<i32, Status> {
        match self.amount.trim().parse::<i32>() {
            Ok(val) if val > 0 => Ok(val),
            _ => Err(Status::error("Wrong value!")),
        }
    }

    fn check_status<T>(&mut self, result: Result<T, Status>) -> Option<T> {
        match result {
            Ok(val) => Some(val),
            Err(status) => {
                self.status = status;
                None
            }
        }
    }

    fn launch_game(&mut self) {
        if let Some(session) = &self.current_session {
            match std::process::Command::new(&self.app_config.dnf_exe_path)
                .arg(&session.token)
                .spawn()
            {
                Ok(_) => {
                    info!("launching game");
                    self.status = Status::success("Launching Game...");
                }
                Err(err) => {
                    error!("failed to launch game: {err}");
                    self.status = Status::error(format!("Launch failed: {err}"));
                }
            }
        }
    }

    fn render_login(&mut self, ui: &mut egui::Ui) {
        let busy = self.action_bind.is_pending();
        ui.add_space(6.0);
        ui.heading("Welcome Back");
        ui.add_space(10.0);

        ui.label(egui::RichText::new("Username").color(Theme::TEXT_MUTED));
        ui.add(
            egui::TextEdit::singleline(&mut self.creds.username)
                .hint_text("Account name")
                .desired_width(ui.available_width())
                .background_color(Theme::SURFACE),
        );
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Password").color(Theme::TEXT_MUTED));
        ui.add(
            egui::TextEdit::singleline(&mut self.creds.password)
                .password(true)
                .hint_text("Password")
                .desired_width(ui.available_width())
                .background_color(Theme::SURFACE),
        );
        ui.add_space(8.0);
        ui.checkbox(&mut self.remember, "Remember me");
        ui.add_space(12.0);

        let login_btn = egui::Button::new(egui::RichText::new("SIGN IN").color(Theme::TEXT))
            .fill(Theme::ACCENT)
            .stroke(egui::Stroke::new(1.0, Theme::ACCENT));
        if ui.add_enabled(!busy, login_btn).clicked() {
            let result = self.login();
            self.check_status(result);
        }

        ui.add_space(8.0);
        let reg_btn = egui::Button::new(egui::RichText::new("CREATE ACCOUNT").color(Theme::TEXT))
            .fill(Theme::ACCENT_SOFT)
            .stroke(egui::Stroke::new(1.0, Theme::ACCENT));
        if ui.add_enabled(!busy, reg_btn).clicked() {
            let result = self.create_account();
            self.check_status(result);
        }
    }

    fn render_dashboard(&mut self, ui: &mut egui::Ui) {
        let busy = self.action_bind.is_pending();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.heading("ACCOUNT DASHBOARD");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let refresh_btn =
                    egui::Button::new(egui::RichText::new("Refresh").color(Theme::TEXT))
                        .fill(Theme::SURFACE_ALT);
                if ui.add_enabled(!busy, refresh_btn).clicked() {
                    let result = self.refresh();
                    self.check_status(result);
                }
            });
        });
        ui.add_space(6.0);

        let cera = self.current_session.as_ref().map(|s| s.cera).unwrap_or(0);
        ui.label(egui::RichText::new(format!("Cera: {cera}")).color(Theme::TEXT_MUTED));
        ui.add_space(6.0);

        egui::Frame::new()
            .fill(Theme::SURFACE)
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(170.0)
                    .show(ui, |ui| {
                        if let Some(session) = &self.current_session {
                            for (idx, character) in session.characters.iter().enumerate() {
                                let label = format!(
                                    "LVL {} | {} | {} | Gold: {}",
                                    character.level, character.job, character.name, character.money
                                );
                                let selected = self.selected_char == Some(idx);
                                if ui.selectable_label(selected, label).clicked() {
                                    self.selected_char = Some(idx);
                                }
                            }
                        }
                    });
            });

        ui.add_space(10.0);
        ui.label(egui::RichText::new("CURRENCY MANAGEMENT").color(Theme::TEXT_MUTED));
        ui.add_space(6.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.amount)
                .hint_text("Amount")
                .desired_width(ui.available_width())
                .background_color(Theme::SURFACE),
        );
        ui.add_space(10.0);
        let button_height = ui.spacing().interact_size.y;
        ui.columns(2, |cols| {
            let gold_btn = egui::Button::new(egui::RichText::new("SEND GOLD").color(Theme::TEXT))
                .fill(Theme::ACCENT);
            let gold_size = egui::vec2(cols[0].available_width(), button_height);
            let response = cols[0].add_enabled_ui(!busy, |ui| {
                ui.add_sized(gold_size, gold_btn)
            });
            if response.inner.on_hover_text("Send gold to selected character").clicked() {
                let result = self.send_gold();
                self.check_status(result);
            }

            let cera_btn = egui::Button::new(egui::RichText::new("SEND CERA").color(Theme::TEXT))
                .fill(Theme::ACCENT);
            let cera_size = egui::vec2(cols[1].available_width(), button_height);
            let response = cols[1].add_enabled_ui(!busy, |ui| {
                ui.add_sized(cera_size, cera_btn)
            });
            if response.inner.on_hover_text("Send cera to account").clicked() {
                let result = self.send_cera();
                self.check_status(result);
            }
        });

        ui.add_space(12.0);
        let play_btn = egui::Button::new(egui::RichText::new("PLAY GAME").color(Theme::TEXT))
            .fill(Theme::ACCENT);
        if ui.add_enabled(!busy, play_btn).clicked() {
            self.launch_game();
        }

        ui.add_space(6.0);
        if ui
            .add_enabled(!busy, egui::Button::new("SWITCH ACCOUNT"))
            .clicked()
        {
            self.screen = Screen::Login;
        }
    }

    fn paint_lightning(&self, painter: egui::Painter, rect: egui::Rect, time: f32) {
        let base_y = rect.center().y;
        let width = rect.width().max(1.0);
        let bolts = 2;
        let segments = 16;
        for bolt in 0..bolts {
            let seed = time * 0.9 + bolt as f32 * 7.3;
            let mut points = Vec::with_capacity(segments + 1);
            for i in 0..=segments {
                let t = i as f32 / segments as f32;
                let x = rect.left() + t * width;
                let jitter = self.hash(seed + i as f32 * 1.7) - 0.5;
                let flicker = (time * 12.0 + bolt as f32).sin() * 0.5 + 0.5;
                let amp = rect.height() * (0.25 + 0.55 * flicker);
                let y = base_y + jitter * amp;
                points.push(egui::pos2(x, y));
            }
            let alpha = (0.25 + 0.35 * (time * 7.0 + bolt as f32).sin().abs()).clamp(0.2, 0.7);
            let glow = egui::Stroke::new(4.0, Theme::ACCENT_SOFT.gamma_multiply(alpha * 0.6));
            let mid = egui::Stroke::new(2.5, Theme::ACCENT.gamma_multiply(alpha * 0.8));
            let core = egui::Stroke::new(1.2, Theme::ACCENT.gamma_multiply(alpha + 0.2));
            painter.add(egui::Shape::line(points.clone(), glow));
            painter.add(egui::Shape::line(points.clone(), mid));
            painter.add(egui::Shape::line(points, core));
        }
    }

    fn hash(&self, x: f32) -> f32 {
        (x.sin() * 43_758.545).fract()
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.plugin_or_default::<EguiAsyncPlugin>();
        self.process_async(ctx);
        Theme::apply(ctx);
        ctx.request_repaint_after_secs(1.0 / 60.0);
        ctx.style_mut(|style| {
            style.spacing.interact_size = egui::vec2(140.0, 32.0);
            style.spacing.item_spacing = egui::vec2(10.0, 10.0);
            style.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(16.0));
            style.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let max_width = ui.available_width().min(420.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width(max_width);
                egui::Frame::new()
                    .fill(Theme::BG_ALT)
                    .corner_radius(egui::CornerRadius::same(12))
                    .inner_margin(egui::Margin::symmetric(20, 18))
                    .show(ui, |ui| {
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("DNF")
                                    .color(Theme::ACCENT)
                                    .strong()
                                    .size(18.0),
                            );
                            ui.label(
                                egui::RichText::new("LAUNCHER")
                                    .color(Theme::TEXT)
                                    .strong()
                                    .size(18.0),
                            );
                        });
                        let lightning_height = 18.0;
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), lightning_height),
                            egui::Sense::hover(),
                        );
                        self.paint_lightning(
                            ui.painter_at(rect),
                            rect,
                            ui.input(|i| i.time) as f32,
                        );
                        ui.add_space(10.0);
                        match self.screen {
                            Screen::Login => self.render_login(ui),
                            Screen::Dashboard => self.render_dashboard(ui),
                        }
                    });
            });
        });

        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(Theme::BG_ALT)
                    .inner_margin(egui::Margin::symmetric(16, 8)),
            )
            .show(ctx, |ui| {
                let color = match self.status.kind {
                    StatusKind::Info => Theme::TEXT_MUTED,
                    StatusKind::Success => Theme::SUCCESS,
                    StatusKind::Error => Theme::ERROR,
                };
                ui.label(egui::RichText::new(&self.status.message).color(color));
            });
    }
}

impl Status {
    fn success(message: impl Into<String>) -> Self {
        Self {
            kind: StatusKind::Success,
            message: message.into(),
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            kind: StatusKind::Error,
            message: message.into(),
        }
    }
}
