use anyhow::Result;
use clap::Parser;

use interpreter::cli::Cli;
use interpreter::config::{self, Config};
use interpreter::{init_tui, llm_client, model_list, shell};

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

    if cli.model_list {
        return model_list::run();
    }

    if cli.init {
        let path = cli
            .config
            .clone()
            .or_else(config::default_config_path)
            .ok_or_else(|| anyhow::anyhow!("could not determine a config path; pass --config"))?;
        return init_tui::run(path, config);
    }

    let shell_ctx = shell::ShellContext::detect()?;
    let command = llm_client::generate_command(&config, &shell_ctx, &cli.input()).await?;

    println!("{command}");
    Ok(())
}
