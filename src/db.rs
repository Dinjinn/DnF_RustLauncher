use anyhow::{Context, Result, bail};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rsa::traits::{PrivateKeyParts, PublicKeyParts};
use rsa::{pkcs8::DecodePrivateKey, BigUint, RsaPrivateKey};
use sqlx::{Connection, MySqlConnection, Row};

use crate::config::AppConfig;

pub struct Db {
    main_url: String,
    billing_url: String,
    chara_url: String,
    inventory_url: String,
    login_url: String,
    private_key: RsaPrivateKey,
}

#[derive(Clone, Copy)]
pub enum DbPool {
    Main,
    Billing,
    Chara,
    Inventory,
    Login,
}

#[derive(Clone, Debug)]
pub struct Character {
    pub id: i32,
    pub name: String,
    pub level: i32,
    pub job: JobName,
    pub money: i64,
}

pub struct LoginSession {
    pub uid: i32,
    pub token: String,
    pub characters: Vec<Character>,
    pub cera: i64,
}

#[derive(Clone, Debug)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Copy, Debug)]
pub enum JobName {
    MaleSlayer,
    FemaleFighter,
    MaleGunner,
    FemaleMage,
    MalePriest,
    FemaleGunner,
    Thief,
    MaleFighter,
    MaleMage,
    FemalePriest,
    FemaleSlayer,
    Unknown,
}

impl JobName {
    pub fn from_id(job_id: i32) -> Self {
        match job_id {
            0 => Self::MaleSlayer,
            1 => Self::FemaleFighter,
            2 => Self::MaleGunner,
            3 => Self::FemaleMage,
            4 => Self::MalePriest,
            5 => Self::FemaleGunner,
            6 => Self::Thief,
            7 => Self::MaleFighter,
            8 => Self::MaleMage,
            9 => Self::FemalePriest,
            10 => Self::FemaleSlayer,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::MaleSlayer => "Male Slayer",
            Self::FemaleFighter => "Female Fighter",
            Self::MaleGunner => "Male Gunner",
            Self::FemaleMage => "Female Mage",
            Self::MalePriest => "Male Priest",
            Self::FemaleGunner => "Female Gunner",
            Self::Thief => "Thief",
            Self::MaleFighter => "Male Fighter",
            Self::MaleMage => "Male Mage",
            Self::FemalePriest => "Female Priest",
            Self::FemaleSlayer => "Female Slayer",
            Self::Unknown => "Unknown",
        }
    }
}

impl std::fmt::Display for JobName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Db {
    pub fn new(cfg: &AppConfig) -> Result<Self> {
        let private_key_pem = include_str!("key.txt");
        let private_key = RsaPrivateKey::from_pkcs8_pem(private_key_pem)?;
        Ok(Self {
            main_url: cfg.db_main_url.clone(),
            billing_url: cfg.db_billing_url.clone(),
            chara_url: cfg.db_char_url.clone(),
            inventory_url: cfg.db_inventory_url.clone(),
            login_url: cfg.db_login_url.clone(),
            private_key,
        })
    }

    pub async fn send_gold(&self, char_id: i32, amount: i32) -> Result<()> {
        tracing::info!("db: send gold request");
        let mut conn = self.get_conn(DbPool::Inventory).await?;
        sqlx::query("UPDATE `inventory` SET money = money + ? WHERE charac_no = ?")
            .bind(amount)
            .bind(char_id)
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn send_cera(&self, uid: i32, amount: i32) -> Result<()> {
        tracing::info!("db: send cera request");
        let mut conn = self.get_conn(DbPool::Billing).await?;
        sqlx::query(
            "INSERT INTO `cash_cera` (`account`, `cera`, `mod_tran`, `mod_date`, `reg_date`) \
             VALUES (?, ?, 1, NOW(), NOW()) \
             ON DUPLICATE KEY UPDATE cera = cera + ?",
        )
        .bind(uid)
        .bind(amount)
        .bind(amount)
        .execute(&mut conn)
        .await?;
        Ok(())
    }

    pub async fn perform_login(&self, username: &str, password: &str) -> Result<LoginSession> {
        tracing::debug!("db: login attempt");
        let mut conn = self.get_conn(DbPool::Main).await?;
        let row = sqlx::query("SELECT uid, password FROM accounts WHERE accountname = ?")
            .bind(username)
            .fetch_optional(&mut conn)
            .await?
            .context("User not found")?;
        let uid: i32 = row.try_get("uid").context("Missing uid")?;
        let stored_hash = row.try_get::<Vec<u8>, _>("password")?;
        if !check_password(password, &stored_hash) {
            bail!("Invalid password");
        }

        let mut billing_conn = self.get_conn(DbPool::Billing).await?;
        let cera_row = sqlx::query("SELECT cera FROM cash_cera WHERE account = ?")
            .bind(uid)
            .fetch_optional(&mut billing_conn)
            .await?;
        let cera = cera_row
            .and_then(|r| r.try_get::<i64, _>("cera").ok())
            .unwrap_or(0);

        let mut chara_conn = self.get_conn(DbPool::Chara).await?;
        let rows = sqlx::query(
            "SELECT c.charac_no, c.charac_name, c.lev, c.job, i.money \
             FROM charac_info c \
             LEFT JOIN taiwan_cain_2nd.inventory i ON c.charac_no = i.charac_no \
             WHERE c.m_id = ? AND c.delete_flag = 0",
        )
        .bind(uid)
        .fetch_all(&mut chara_conn)
        .await?;
        let characters = rows
            .into_iter()
            .map(|row| {
                let job_id: i32 = row.try_get("job").unwrap_or_default();
                Character {
                    id: row.try_get("charac_no").unwrap_or_default(),
                    name: row.try_get("charac_name").unwrap_or_default(),
                    level: row.try_get("lev").unwrap_or_default(),
                    job: JobName::from_id(job_id),
                    money: row.try_get("money").unwrap_or(0),
                }
            })
            .collect::<Vec<_>>();

        Ok(LoginSession {
            uid,
            token: self.generate_login_token(uid)?,
            characters,
            cera,
        })
    }

    pub async fn create_account(&self, username: &str, password: &str) -> Result<()> {
        tracing::info!("db: create account request");
        let mut conn = self.get_conn(DbPool::Main).await?;
        let mut tx = conn.begin().await?;
        let existing: Option<i32> =
            sqlx::query_scalar("SELECT uid FROM accounts WHERE accountname = ?")
                .bind(username)
                .fetch_optional(&mut *tx)
                .await?;
        if existing.is_some() {
            bail!("Account name already exists!");
        }

        let hashed_password = hash_password(password);
        // Accounts and related inserts are kept in a transaction.
        sqlx::query("INSERT INTO accounts (accountname, password, qq) VALUES (?, ?, ?)")
            .bind(username)
            .bind(&hashed_password)
            .bind(password)
            .execute(&mut *tx)
            .await?;

        let uid: i32 = sqlx::query_scalar("SELECT uid FROM accounts WHERE accountname = ?")
            .bind(username)
            .fetch_one(&mut *tx)
            .await
            .context("UID Fail")?;

        sqlx::query("INSERT INTO limit_create_character (m_id) VALUES (?)")
            .bind(uid)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO member_info (m_id, user_id) VALUES (?, ?)")
            .bind(uid)
            .bind(uid.to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO member_white_account (m_id) VALUES (?)")
            .bind(uid)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        let mut login_conn = self.get_conn(DbPool::Login).await?;
        sqlx::query("INSERT INTO member_login (m_id) VALUES (?)")
            .bind(uid)
            .execute(&mut login_conn)
            .await?;

        Ok(())
    }

    async fn get_conn(&self, pool: DbPool) -> Result<MySqlConnection> {
        let url = match pool {
            DbPool::Main => self.main_url.as_str(),
            DbPool::Billing => self.billing_url.as_str(),
            DbPool::Chara => self.chara_url.as_str(),
            DbPool::Inventory => self.inventory_url.as_str(),
            DbPool::Login => self.login_url.as_str(),
        };
        tracing::debug!("db: open connection");
        Ok(MySqlConnection::connect(url).await?)
    }

    fn generate_login_token(&self, uid: i32) -> Result<String> {
        let pre_str = "1FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00";
        let next_str = "010101010101010101010101010101010101010101010101010101010101010155914510010403030101";
        let uid_hex = format!("{:08X}", uid as u32);
        let src_str = format!("{pre_str}{uid_hex}{next_str}");
        let message = BigUint::parse_bytes(src_str.as_bytes(), 16).context("Hex fail")?;
        let encrypted = message.modpow(self.private_key.d(), self.private_key.n());
        Ok(BASE64.encode(hex::decode(encrypted.to_str_radix(16))?))
    }
}

fn hash_password(password: &str) -> String {
    let digest = md5::compute(password);
    format!("{:x}", digest)
}

fn check_password(password: &str, stored_hash: &[u8]) -> bool {
    hash_password(password).as_bytes() == stored_hash
}
