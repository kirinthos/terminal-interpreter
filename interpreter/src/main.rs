use anyhow::Result;
use clap::Parser;

use interpreter::cli::Cli;
use interpreter::config::Config;
use interpreter::{llm_client, shell};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let config = Config::load(cli.config.as_deref())?.with_overrides(&cli);

    let shell_ctx = shell::ShellContext::detect(config.history_read_limit)?;
    let command = llm_client::generate_command(&config, &shell_ctx, &cli.input()).await?;

    println!("{command}");
    Ok(())
}
