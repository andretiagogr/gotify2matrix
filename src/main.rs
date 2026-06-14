use std::path::PathBuf;
use clap::Parser;

mod client;
pub mod config;
mod gotify_client;
pub mod session;
mod verify;

#[derive(clap::Subcommand)]
enum Command {
    /// Wait for incoming device verifications
    Verify,
}

#[derive(Parser)]
#[command(version)]
struct Options {
    #[arg(short = 'c', long = "config")]
    config_file: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Install the process-wide rustls crypto provider once, at startup, before any
    // TLS consumer (reqwest, the gotify websocket) is constructed. The error case
    // only signals a provider was already installed, which is harmless to ignore.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let options = Options::parse();

    let config_file = match options.config_file {
        Some(file) => file,
        None => dirs::config_local_dir()
            .expect("Config dir does not exist")
            .join("gotify2matrix")
            .join("config.toml"),
    };
    let config = config::Config::read(config_file)?;
    match options.command {
        Some(Command::Verify) => verify::run(config).await?,
        _ => client::run(config).await?,
    }
    Ok(())
}
