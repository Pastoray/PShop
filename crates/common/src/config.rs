//! Tiny 12-factor config helpers. All services are configured purely by env.

use std::{env, fmt::Debug, str::FromStr};

pub fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Parse an env var, falling back to `default` when unset.
/// A *set but malformed* value is a deploy bug, so we fail fast instead of
/// silently running with a default.
pub fn env_parse<T>(key: &str, default: T) -> T
where
    T: FromStr,
    T::Err: Debug,
{
    match env::var(key) {
        Ok(v) => v
            .parse()
            .unwrap_or_else(|e| panic!("invalid value for {key}={v:?}: {e:?}")),
        Err(_) => default,
    }
}

pub fn env_required(key: &str) -> anyhow::Result<String> {
    env::var(key).map_err(|_| anyhow::anyhow!("required environment variable {key} is not set"))
}
