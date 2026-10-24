//! Secrets are fetched from AWS Secrets Manager at process start. Nothing
//! sensitive is baked into images, ConfigMaps or env (except the dummy LocalStack
//! credentials needed to reach Secrets Manager itself).
//!
//! `SECRETS_BACKEND=env` bypasses Secrets Manager for unit/dev runs.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use aws_sdk_secretsmanager::error::DisplayErrorContext;
use serde::{de::DeserializeOwned, Deserialize};

use crate::{config::env_or, util::retry};

#[derive(Clone, Deserialize)]
pub struct PostgresSecret {
    pub username: String,
    pub password: String,
    pub host: String,
    #[serde(default = "default_pg_port")]
    pub port: u16,
    pub dbname: String,
}

fn default_pg_port() -> u16 {
    5432
}

// Never let a secret reach logs through `{:?}`.
impl std::fmt::Debug for PostgresSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresSecret")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("dbname", &self.dbname)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Deserialize)]
pub struct AppKeys {
    /// HMAC key used to sign invoices so they are tamper-evident.
    pub invoice_signing_key: String,
}

impl std::fmt::Debug for AppKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AppKeys(<redacted>)")
    }
}

fn use_env_backend() -> bool {
    env_or("SECRETS_BACKEND", "aws") == "env"
}

async fn fetch_json<T: DeserializeOwned>(secret_id: &str) -> Result<T> {
    let cfg = crate::aws::sdk_config().await;
    let client = aws_sdk_secretsmanager::Client::new(&cfg);
    // Terraform may still be applying when pods start, so retry generously.
    let out = retry(
        "secretsmanager:GetSecretValue",
        60,
        Duration::from_millis(500),
        || async {
            client
                .get_secret_value()
                .secret_id(secret_id)
                .send()
                .await
                .map_err(|e| anyhow!("{}", DisplayErrorContext(&e)))
        },
    )
    .await
    .with_context(|| format!("fetching secret {secret_id}"))?;
    let raw = out
        .secret_string()
        .ok_or_else(|| anyhow!("secret {secret_id} has no SecretString"))?;
    serde_json::from_str(raw)
        .with_context(|| format!("secret {secret_id} is not the expected JSON"))
}

pub async fn load_postgres() -> Result<PostgresSecret> {
    if use_env_backend() {
        return Ok(PostgresSecret {
            username: env_or("PG_USER", "pshop"),
            password: env_or("PG_PASSWORD", "pshop"),
            host: env_or("PG_HOST", "localhost"),
            port: env_or("PG_PORT", "5432").parse()?,
            dbname: env_or("PG_DB", "pshop"),
        });
    }
    fetch_json(&env_or("PG_SECRET_ID", "pshop/postgres")).await
}

pub async fn load_app_keys() -> Result<AppKeys> {
    if use_env_backend() {
        return Ok(AppKeys {
            invoice_signing_key: env_or("INVOICE_SIGNING_KEY", "dev-only"),
        });
    }
    fetch_json(&env_or("APP_KEYS_SECRET_ID", "pshop/app-keys")).await
}
