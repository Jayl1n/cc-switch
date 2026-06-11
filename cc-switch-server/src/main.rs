//! CC Switch Server — Headless standalone binary
//!
//! Embeddable proxy server for AI coding tools (Claude, Codex, Gemini).
//! Runs as a subprocess, controlled via JSON-over-stdout protocol.
//!
//! ## Usage
//!
//! ```bash
//! cc-switch-server --config-dir /path/to/config --port 0
//! ```
//!
//! Outputs JSON status to stdout, then blocks until SIGTERM/SIGINT.

use cc_switch_core::{Core, NoopAuthProvider, NoopEvents};
use clap::Parser;
use std::sync::Arc;

/// Headless CC Switch proxy server.
#[derive(Parser, Debug)]
#[command(name = "cc-switch-server", version, about)]
struct Args {
    /// Configuration directory (default: ~/.cc-switch/)
    #[arg(long, short = 'd')]
    config_dir: Option<String>,

    /// Proxy listen port (0 = OS assigns a free port)
    #[arg(long, short = 'p', default_value = "0")]
    port: u16,

    /// Listen address
    #[arg(long, default_value = "127.0.0.1")]
    listen_addr: Option<String>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Start with Live config takeover (rewrites tool config files to point at proxy)
    #[arg(long)]
    takeover: bool,

    /// Output format for status: json or line
    #[arg(long, default_value = "json")]
    output: String,
}

fn main() {
    let args = Args::parse();

    // Initialize logger (writes to stderr so stdout stays clean for JSON protocol)
    env_logger::Builder::new()
        .filter_level(
            args.log_level
                .parse()
                .unwrap_or(log::LevelFilter::Info),
        )
        .format_timestamp_secs()
        .init();

    // Use tokio runtime
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(async move {
        run_server(args).await;
    });
}

async fn run_server(args: Args) {
    let config_dir = args.config_dir.map(std::path::PathBuf::from);

    // Build Core
    let core = Core::builder()
        .events(Arc::new(NoopEvents))
        .auth_provider(Arc::new(NoopAuthProvider))
        .maybe_config_dir(config_dir)
        .build()
        .unwrap_or_else(|e| {
            eprintln!("ERROR: Failed to initialize: {e}");
            std::process::exit(1);
        });

    // Set listen port if specified (and not default 0)
    if let Some(ref addr) = args.listen_addr {
        let db = core.db();
        let mut config = db
            .get_proxy_config()
            .await
            .expect("Failed to read proxy config");
        config.listen_port = args.port;
        config.listen_address = addr.clone();
        db.update_proxy_config(config)
            .await
            .expect("Failed to set proxy config");
    }

    // Start proxy
    let result = if args.takeover {
        core.start_proxy_with_takeover().await
    } else {
        core.start_proxy().await
    };

    let info = result.unwrap_or_else(|e| {
        eprintln!("ERROR: Failed to start proxy: {e}");
        std::process::exit(1);
    });

    // Output status to stdout (for the parent process to read)
    let status = serde_json::json!({
        "status": "running",
        "address": info.address,
        "port": info.port,
        "started_at": info.started_at,
        "takeover": args.takeover,
    });

    if args.output == "json" {
        println!("{}", serde_json::to_string(&status).unwrap());
    } else {
        println!("CC Switch proxy running on {}:{}", info.address, info.port);
    }

    // Wait for shutdown signal
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl-c");

    eprintln!("Shutting down...");

    // Graceful shutdown
    if args.takeover {
        core.stop_proxy_with_restore()
            .await
            .unwrap_or_else(|e| eprintln!("WARN: stop with restore failed: {e}"));
    } else {
        core.stop_proxy()
            .await
            .unwrap_or_else(|e| eprintln!("WARN: stop failed: {e}"));
    }

    let shutdown_status = serde_json::json!({
        "status": "stopped",
        "port": info.port,
    });
    println!("{}", serde_json::to_string(&shutdown_status).unwrap());
}
