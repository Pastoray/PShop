//! PShop traffic generator.
//!
//! OPEN-LOOP: requests are launched on a schedule regardless of whether earlier
//! ones have completed. A closed-loop tool (N workers each waiting for a reply)
//! slows down exactly when the server does and hides the very latency you are
//! trying to measure ("coordinated omission"). Here, if the server stalls,
//! in-flight requests pile up and the numbers say so.
//!
//! Profile: linear ramp from 10% to `--rps` over `--ramp` s, then hold until
//! `--duration` s. Hot-SKU skew makes a few products take most of the traffic,
//! which is what actually stresses stock contention in Redis.

use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use anyhow::Result;
use clap::Parser;
use hdrhistogram::Histogram;
use rand::RngExt;
use tokio::sync::{mpsc, Semaphore};

#[derive(Parser, Debug, Clone)]
#[command(name = "loadgen", about = "Open-loop order flood for PShop Engine")]
struct Args {
    /// Base URL of the ingress (Nginx).
    #[arg(long, default_value = "http://localhost:8080")]
    url: String,
    /// Target requests per second at full load.
    #[arg(long, default_value_t = 5000)]
    rps: u32,
    /// Total run time, seconds.
    #[arg(long, default_value_t = 60)]
    duration: u64,
    /// Seconds to ramp from 10% to full rps.
    #[arg(long, default_value_t = 10)]
    ramp: u64,
    /// Number of distinct SKUs (SKU-0001..). Must not exceed the seeded catalogue.
    #[arg(long, default_value_t = 100)]
    skus: u32,
    /// Distinct synthetic users.
    #[arg(long, default_value_t = 50_000)]
    users: u32,
    /// Hot-key skew: 1.0 = uniform, higher = more traffic on the first SKUs.
    #[arg(long, default_value_t = 3.0)]
    skew: f64,
    /// Percent of requests that replay a previous Idempotency-Key.
    #[arg(long, default_value_t = 0.5)]
    dup_pct: f64,
    /// Client-side cap on concurrent in-flight requests.
    #[arg(long, default_value_t = 20_000)]
    max_inflight: usize,
    /// Per-request timeout, ms.
    #[arg(long, default_value_t = 5000)]
    timeout_ms: u64,
    /// Fail (exit 1) if p99 latency of accepted requests exceeds this many ms.
    #[arg(long)]
    slo_p99_ms: Option<f64>,
}

/// Target rate at `t` seconds into the run.
fn target_rps(a: &Args, t: f64) -> f64 {
    let full = f64::from(a.rps);
    if a.ramp == 0 || t >= a.ramp as f64 {
        return full;
    }
    full * (0.1 + 0.9 * (t / a.ramp as f64))
}

/// Skewed SKU pick: `u^skew` concentrates mass near index 0.
fn pick_sku(rng: &mut impl RngExt, skus: u32, skew: f64) -> u32 {
    let u: f64 = rng.random();
    ((f64::from(skus) * u.powf(skew)) as u32).min(skus - 1) + 1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Class {
    Accepted,   // 202
    SoldOut,    // 409
    Throttled,  // 429
    Overloaded, // 503
    Other4xx,
    Other5xx,
    Error, // transport error / timeout
}

impl Class {
    const ALL: [Class; 7] = [
        Class::Accepted,
        Class::SoldOut,
        Class::Throttled,
        Class::Overloaded,
        Class::Other4xx,
        Class::Other5xx,
        Class::Error,
    ];
    fn of(status: u16) -> Class {
        match status {
            202 => Class::Accepted,
            409 => Class::SoldOut,
            429 => Class::Throttled,
            503 => Class::Overloaded,
            400..=499 => Class::Other4xx,
            _ => Class::Other5xx,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Class::Accepted => "202",
            Class::SoldOut => "409",
            Class::Throttled => "429",
            Class::Overloaded => "503",
            Class::Other4xx => "4xx",
            Class::Other5xx => "5xx",
            Class::Error => "err",
        }
    }
}

struct Sample {
    class: Class,
    micros: u64,
}

fn hist() -> Histogram<u64> {
    // 1 us .. 60 s, 3 significant digits.
    Histogram::new_with_bounds(1, 60_000_000, 3).expect("valid histogram bounds")
}

fn ms(h: &Histogram<u64>, q: f64) -> f64 {
    h.value_at_quantile(q) as f64 / 1000.0
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    anyhow::ensure!(
        args.skus >= 1 && args.users >= 1,
        "--skus and --users must be >= 1"
    );
    println!(
        "loadgen -> {}  target={} rps  duration={}s  ramp={}s  skus={}  skew={}  dup={}%",
        args.url, args.rps, args.duration, args.ramp, args.skus, args.skew, args.dup_pct
    );

    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(4096)
        .tcp_nodelay(true)
        .timeout(Duration::from_millis(args.timeout_ms))
        .build()?;
    let endpoint = format!("{}/v1/orders", args.url.trim_end_matches('/'));

    let permits = Arc::new(Semaphore::new(args.max_inflight));
    let inflight = Arc::new(AtomicU64::new(0));
    let launched = Arc::new(AtomicU64::new(0));
    let dropped = Arc::new(AtomicU64::new(0)); // client-side saturation, NOT server errors
    let (tx, mut rx) = mpsc::unbounded_channel::<Sample>();

    // ---- scheduler -------------------------------------------------------------
    let sched = {
        let (args, client, permits, inflight, launched, dropped) = (
            args.clone(),
            client.clone(),
            Arc::clone(&permits),
            Arc::clone(&inflight),
            Arc::clone(&launched),
            Arc::clone(&dropped),
        );
        tokio::spawn(async move {
            const TICK: Duration = Duration::from_millis(10);
            let start = Instant::now();
            let mut tick = tokio::time::interval(TICK);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Burst); // catch up, don't lose rate
            let mut carry = 0.0f64;
            let mut recent: VecDeque<(String, String, String)> = VecDeque::with_capacity(1024);

            while start.elapsed().as_secs() < args.duration {
                tick.tick().await;
                let t = start.elapsed().as_secs_f64();
                carry += target_rps(&args, t) * TICK.as_secs_f64();
                let n = carry.floor() as u64;
                carry -= n as f64;

                for _ in 0..n {
                    // RNG is !Send: build the request params in a block with no .await inside.
                    let (user, sku, key, is_dup) = {
                        let mut rng = rand::rng();
                        if !recent.is_empty() && rng.random::<f64>() * 100.0 < args.dup_pct {
                            let i = rng.random_range(0..recent.len());
                            let (k, s, u) = recent[i].clone();
                            (u, s, k, true)
                        } else {
                            let user = format!("u-{}", rng.random_range(0..args.users));
                            let sku =
                                format!("SKU-{:04}", pick_sku(&mut rng, args.skus, args.skew));
                            (user, sku, uuid::Uuid::new_v4().to_string(), false)
                        }
                    };
                    if !is_dup {
                        if recent.len() == 1024 {
                            recent.pop_front();
                        }
                        recent.push_back((key.clone(), sku.clone(), user.clone()));
                    }

                    let Ok(permit) = Arc::clone(&permits).try_acquire_owned() else {
                        dropped.fetch_add(1, Ordering::Relaxed);
                        continue;
                    };
                    launched.fetch_add(1, Ordering::Relaxed);
                    inflight.fetch_add(1, Ordering::Relaxed);

                    let (client, endpoint, tx, inflight) = (
                        client.clone(),
                        endpoint.clone(),
                        tx.clone(),
                        Arc::clone(&inflight),
                    );
                    tokio::spawn(async move {
                        let body = serde_json::json!({ "user_id": user, "sku": sku, "qty": 1 });
                        let t0 = Instant::now();
                        let class = match client
                            .post(&endpoint)
                            .header("Idempotency-Key", key)
                            .json(&body)
                            .send()
                            .await
                        {
                            Ok(resp) => {
                                let class = Class::of(resp.status().as_u16());
                                let _ = resp.bytes().await; // drain so the connection is reused
                                class
                            }
                            Err(_) => Class::Error,
                        };
                        let micros = t0.elapsed().as_micros() as u64;
                        inflight.fetch_sub(1, Ordering::Relaxed);
                        drop(permit);
                        let _ = tx.send(Sample { class, micros });
                    });
                }
            }
        })
    };

    // ---- aggregator + per-second report ------------------------------------------
    let mut all_hist = hist();
    let mut ok_hist = hist();
    let mut win_hist = hist();
    let mut totals = std::collections::HashMap::<Class, u64>::new();
    let mut win = std::collections::HashMap::<Class, u64>::new();
    let started = Instant::now();
    let mut report = tokio::time::interval(Duration::from_secs(1));
    report.tick().await;
    let mut sched = sched;
    let mut sched_done = false;
    let mut ctrl_c = Box::pin(tokio::signal::ctrl_c());

    loop {
        tokio::select! {
            Some(s) = rx.recv() => {
                let m = s.micros.clamp(1, 60_000_000);
                let _ = all_hist.record(m);
                if s.class == Class::Accepted { let _ = ok_hist.record(m); let _ = win_hist.record(m); }
                *totals.entry(s.class).or_default() += 1;
                *win.entry(s.class).or_default() += 1;
            }
            _ = report.tick() => {
                let t = started.elapsed().as_secs();
                let n = |c: Class| win.get(&c).copied().unwrap_or(0);
                println!(
                    "t={:>3}s target={:>5.0} done/s={:>5} 202={:>5} 409={:>4} 503={:>4} 429={:>3} err={:>3} p50={:>6.2}ms p99={:>7.2}ms inflight={:>5} dropped={}",
                    t, target_rps(&args, t as f64), win.values().sum::<u64>(),
                    n(Class::Accepted), n(Class::SoldOut), n(Class::Overloaded), n(Class::Throttled), n(Class::Error),
                    if win_hist.is_empty() { 0.0 } else { ms(&win_hist, 0.5) },
                    if win_hist.is_empty() { 0.0 } else { ms(&win_hist, 0.99) },
                    inflight.load(Ordering::Relaxed), dropped.load(Ordering::Relaxed),
                );
                win.clear();
                win_hist.reset();
            }
            r = &mut sched, if !sched_done => { r?; sched_done = true; }
            _ = &mut ctrl_c => { println!("interrupted; draining..."); sched.abort(); break; }
        }
        if sched_done && inflight.load(Ordering::Relaxed) == 0 && rx.is_empty() {
            break;
        }
    }

    // ---- summary -------------------------------------------------------------------
    let elapsed = started.elapsed().as_secs_f64().max(0.001);
    let total: u64 = totals.values().sum();
    println!("\n==== summary ({elapsed:.1}s) ====");
    println!(
        "launched={}  completed={}  achieved={:.0} req/s  client-dropped={}",
        launched.load(Ordering::Relaxed),
        total,
        total as f64 / elapsed,
        dropped.load(Ordering::Relaxed)
    );
    for c in Class::ALL {
        if let Some(n) = totals.get(&c) {
            println!(
                "  {:>4}: {:>9}  ({:.2}%)",
                c.label(),
                n,
                *n as f64 * 100.0 / total.max(1) as f64
            );
        }
    }
    if !ok_hist.is_empty() {
        println!(
            "accepted (202) latency ms: p50={:.2} p90={:.2} p99={:.2} p99.9={:.2} max={:.2}",
            ms(&ok_hist, 0.5),
            ms(&ok_hist, 0.9),
            ms(&ok_hist, 0.99),
            ms(&ok_hist, 0.999),
            ok_hist.max() as f64 / 1000.0
        );
    }
    if !all_hist.is_empty() {
        println!(
            "all responses  latency ms: p50={:.2} p99={:.2}",
            ms(&all_hist, 0.5),
            ms(&all_hist, 0.99)
        );
    }
    if let (Some(slo), false) = (args.slo_p99_ms, ok_hist.is_empty()) {
        let p99 = ms(&ok_hist, 0.99);
        if p99 > slo {
            println!("SLO FAIL: p99 {p99:.2}ms > {slo}ms");
            std::process::exit(1);
        }
        println!("SLO PASS: p99 {p99:.2}ms <= {slo}ms");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(rps: u32, ramp: u64) -> Args {
        Args::parse_from([
            "loadgen",
            "--rps",
            &rps.to_string(),
            "--ramp",
            &ramp.to_string(),
        ])
    }

    #[test]
    fn ramp_goes_from_10_percent_to_full() {
        let a = args(1000, 10);
        assert!((target_rps(&a, 0.0) - 100.0).abs() < 1e-6);
        assert!((target_rps(&a, 5.0) - 550.0).abs() < 1e-6);
        assert_eq!(target_rps(&a, 10.0), 1000.0);
        assert_eq!(target_rps(&a, 99.0), 1000.0);
        assert_eq!(target_rps(&args(1000, 0), 0.0), 1000.0);
    }

    #[test]
    fn skew_concentrates_on_low_indices_and_stays_in_range() {
        let mut rng = rand::rng();
        let (mut low, n) = (0u32, 20_000u32);
        for _ in 0..n {
            let s = pick_sku(&mut rng, 100, 3.0);
            assert!((1..=100).contains(&s));
            if s <= 10 {
                low += 1;
            }
        }
        // With u^3, P(index < 10) = 0.1^(1/3) ~ 46%; uniform would be 10%.
        assert!(
            low > n / 4,
            "expected strong skew, got {low}/{n} in the first 10 SKUs"
        );
    }

    #[test]
    fn status_classification() {
        assert_eq!(Class::of(202), Class::Accepted);
        assert_eq!(Class::of(503), Class::Overloaded);
        assert_eq!(Class::of(418), Class::Other4xx);
        assert_eq!(Class::of(502), Class::Other5xx);
    }
}
