//! `healthvibe` — one binary, several subcommands.

use clap::{Parser, Subcommand};
use healthvibe_core::config::Config;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "healthvibe",
    version,
    about = "Self-hosted Apple Health dashboard"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the web server.
    Serve,
    /// Probe http://127.0.0.1:<port>/healthz; exit 0 if healthy, 1 otherwise.
    /// Used by the Docker healthcheck (distroless has no curl).
    Healthcheck {
        #[arg(
            long,
            env = "HEALTHVIBE_HEALTHCHECK_ADDR",
            default_value = "127.0.0.1:8000"
        )]
        addr: String,
    },
    /// Write deterministic synthetic demo data for a user (channel `demo`).
    /// Clears prior demo-channel rows for that user first. Not real health data.
    SeedDemo {
        /// User id to fill (default: 1, the first account).
        #[arg(long, default_value = "1")]
        user: i64,
        /// How many local days to generate, ending on `--end`.
        #[arg(long, default_value = "28")]
        days: i64,
        /// Inclusive end date `YYYY-MM-DD` (default: today UTC).
        #[arg(long)]
        end: Option<String>,
        /// RNG seed for reproducibility.
        #[arg(long, default_value = "42")]
        seed: u64,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Serve => {
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_env("RUST_LOG")
                        .unwrap_or_else(|_| "info".into()),
                )
                .init();
            tokio::runtime::Runtime::new()?.block_on(healthvibe_server::serve(Config::from_env()))
        }
        Cmd::Healthcheck { addr } => {
            if probe(&addr).unwrap_or(false) {
                Ok(())
            } else {
                std::process::exit(1)
            }
        }
        Cmd::SeedDemo {
            user,
            days,
            end,
            seed,
        } => {
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_env("RUST_LOG")
                        .unwrap_or_else(|_| "info".into()),
                )
                .init();
            let cfg = Config::from_env();
            tokio::runtime::Runtime::new()?.block_on(async move {
                let db = healthvibe_core::db::open(&cfg).await?;
                let report = healthvibe_core::seed::seed_demo(
                    &db,
                    healthvibe_core::seed::SeedOpts {
                        user_id: user,
                        days,
                        end,
                        rng_seed: seed,
                    },
                )
                .await?;
                println!(
                    "seeded demo data for user {user}: {} readings, {} workouts, {} goal days ({} days, seed {seed})",
                    report.readings, report.workouts, report.goals, report.days
                );
                Ok(())
            })
        }
    }
}

fn probe(addr: &str) -> std::io::Result<bool> {
    let sock = addr
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let mut s = TcpStream::connect_timeout(&sock, Duration::from_secs(3))?;
    s.set_read_timeout(Some(Duration::from_secs(3)))?;
    write!(s, "GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n")?;
    let mut buf = String::new();
    s.read_to_string(&mut buf)?;
    Ok(buf.starts_with("HTTP/1.1 200") || buf.starts_with("HTTP/1.0 200"))
}
