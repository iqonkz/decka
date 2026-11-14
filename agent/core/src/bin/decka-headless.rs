use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;

use decka_core::actions::DryRunner;
use decka_core::{Agent, AgentOptions};

fn usage_error(msg: &str) -> ! {
    eprintln!("{msg}");
    exit(64);
}

fn log_level() -> tracing::Level {
    match std::env::var("DECKA_LOG").unwrap_or_default().to_ascii_lowercase().as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    }
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let mut opts = AgentOptions {
        config_dir: None,
        client_dist: None,
        port_override: None,
        runner: Arc::new(DryRunner), // the real runner arrives in M3
        fixed_pair_code: None,
    };
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().unwrap_or_else(|| usage_error(&format!("{name} needs a value")));
        match arg.as_str() {
            "--version" => {
                println!("decka-headless {}", decka_core::VERSION);
                exit(0);
            }
            "--port" => {
                opts.port_override = Some(value("--port").parse().unwrap_or_else(|_| usage_error("--port must be 0..65535")));
            }
            "--config-dir" => opts.config_dir = Some(PathBuf::from(value("--config-dir"))),
            "--client-dist" => opts.client_dist = Some(PathBuf::from(value("--client-dist"))),
            "--pair-code" => opts.fixed_pair_code = Some(value("--pair-code")),
            "--dry-run" => {}
            other => usage_error(&format!("unknown argument '{other}'")),
        }
    }

    // Logs go to stderr so stdout carries only LISTENING / LISTEN_ERROR / DRY lines.
    tracing_subscriber::fmt().with_max_level(log_level()).with_writer(std::io::stderr).init();

    let agent = match Agent::start(opts).await {
        Ok(a) => a,
        Err(e) => {
            println!("LISTEN_ERROR {e}");
            exit(2);
        }
    };
    let status = agent.status();
    match (status.listening, status.listen_error) {
        (Some(l), _) => println!("LISTENING {}:{}", l.bind, l.port),
        (None, err) => {
            println!("LISTEN_ERROR {}", err.unwrap_or_else(|| "unknown error".to_string()));
            exit(2);
        }
    }

    let _ = tokio::signal::ctrl_c().await;
    agent.shutdown().await;
}
