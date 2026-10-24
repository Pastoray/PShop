//! Demo control plane: simulate the payment provider being slow or down.
//!
//! The mode lives in Redis (`pshop:ctl:payments`) so every billing replica obeys it, and it
//! expires on its own (`MODE_TTL_SECS`) so a forgotten "down" cannot wedge the system. The
//! consumer checks a cached atomic, never Redis, on its hot loop.
//!
//! Disabled unless `CONTROL_ENABLED=true`: it must never be reachable on a real deployment.

use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::Duration,
};

use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use redis::{aio::ConnectionManager, AsyncCommands};
use serde::{Deserialize, Serialize};

const KEY: &str = "pshop:ctl:payments";
const MODE_TTL_SECS: u64 = 600;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Normal,
    Slow,
    Down,
}

impl Mode {
    fn to_u8(self) -> u8 {
        match self {
            Mode::Normal => 0,
            Mode::Slow => 1,
            Mode::Down => 2,
        }
    }
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Mode::Slow,
            2 => Mode::Down,
            _ => Mode::Normal,
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "slow" => Mode::Slow,
            "down" => Mode::Down,
            _ => Mode::Normal,
        }
    }
}

#[derive(Clone)]
pub struct Control {
    mode: Arc<AtomicU8>,
    slow_delay: Duration,
}

impl Control {
    pub fn new(slow_delay: Duration) -> Self {
        Self {
            mode: Arc::new(AtomicU8::new(0)),
            slow_delay,
        }
    }

    pub fn mode(&self) -> Mode {
        Mode::from_u8(self.mode.load(Ordering::Relaxed))
    }

    /// Called by the consumer before it starts each payment.
    pub async fn gate(&self) {
        loop {
            match self.mode() {
                Mode::Normal => return,
                Mode::Slow => {
                    tokio::time::sleep(self.slow_delay).await;
                    return;
                }
                // Hold the delivery unacked: nothing is lost, the queue just grows.
                Mode::Down => tokio::time::sleep(Duration::from_millis(200)).await,
            }
        }
    }

    /// Mirrors the Redis value into the atomic twice a second.
    pub async fn follow(self, mut redis: ConnectionManager) {
        loop {
            let v: redis::RedisResult<Option<String>> = redis.get(KEY).await;
            if let Ok(v) = v {
                let m = v.as_deref().map(Mode::parse).unwrap_or(Mode::Normal);
                self.mode.store(m.to_u8(), Ordering::Relaxed);
                crate::m::PAYMENTS_MODE.set(i64::from(m.to_u8()));
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
}

#[derive(Clone)]
pub struct ApiState {
    pub control: Control,
    pub redis: ConnectionManager,
}

#[derive(Serialize)]
struct ModeBody {
    mode: Mode,
}

#[derive(Deserialize)]
struct SetBody {
    mode: Mode,
}

pub fn router(st: ApiState) -> Router {
    Router::new()
        .route("/v1/control/payments", get(get_mode).put(set_mode))
        .with_state(st)
}

async fn get_mode(State(st): State<ApiState>) -> Json<ModeBody> {
    Json(ModeBody {
        mode: st.control.mode(),
    })
}

async fn set_mode(
    State(mut st): State<ApiState>,
    Json(b): Json<SetBody>,
) -> Result<Json<ModeBody>, StatusCode> {
    let name = match b.mode {
        Mode::Normal => "normal",
        Mode::Slow => "slow",
        Mode::Down => "down",
    };
    st.redis
        .set_ex::<_, _, ()>(KEY, name, MODE_TTL_SECS)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    // Reflect immediately on this replica; the others pick it up within 500 ms.
    st.control.mode.store(b.mode.to_u8(), Ordering::Relaxed);
    Ok(Json(ModeBody { mode: b.mode }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_roundtrip() {
        for m in [Mode::Normal, Mode::Slow, Mode::Down] {
            assert_eq!(Mode::from_u8(m.to_u8()), m);
        }
        assert_eq!(Mode::parse("garbage"), Mode::Normal);
    }

    #[tokio::test]
    async fn normal_does_not_block_and_down_blocks_until_cleared() {
        let c = Control::new(Duration::from_millis(1));
        tokio::time::timeout(Duration::from_millis(50), c.gate())
            .await
            .expect("normal must pass");
        c.mode.store(Mode::Down.to_u8(), Ordering::Relaxed);
        assert!(tokio::time::timeout(Duration::from_millis(100), c.gate())
            .await
            .is_err());
        let c2 = c.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            c2.mode.store(0, Ordering::Relaxed);
        });
        tokio::time::timeout(Duration::from_millis(500), c.gate())
            .await
            .expect("must resume");
    }
}
