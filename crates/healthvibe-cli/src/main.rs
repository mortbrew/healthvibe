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
