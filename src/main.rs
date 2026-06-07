use std::path::PathBuf;
use clap::Parser;

mod client;
pub mod config;
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
