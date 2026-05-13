use std::path::PathBuf;

use clap::Parser;

/// `interpreter` reads a partial shell command line, gathers context from the
/// current shell (kind, history, working directory), and asks an LLM to rewrite
/// it into a fully-formed shell command. The generated command is printed on
/// stdout so a shell wrapper can substitute it back onto the prompt.
#[derive(Debug, Parser)]
#[command(
    name = "interpreter",
    version,
    about = "Rewrite a shell command line into an executable command using an LLM.",
    long_about = None,
)]
pub struct Cli {
    /// The current shell command line to interpret. If omitted, stdin is read.
    #[arg(value_name = "COMMAND")]
    pub command: Vec<String>,

    /// Override the model in `provider/model-name` form
    /// (e.g. `openai/gpt-4o-mini`, `anthropic/claude-opus-4-7`).
    #[arg(short, long, value_name = "PROVIDER/MODEL")]
    pub model: Option<String>,

    /// Path to a configuration file. Reads `$INTERPRETER_CONFIG` when unset;
    /// otherwise falls back to the platform config dir
    /// (e.g. `$XDG_CONFIG_HOME/interpreter/config.json`).
    #[arg(short, long, value_name = "PATH", env = "INTERPRETER_CONFIG")]
    pub config: Option<PathBuf>,

    /// Print the resolved configuration and exit without calling the LLM.
    #[arg(long)]
    pub dry_run: bool,

    /// List available models from each configured provider in the exact
    /// `provider/model-name` form expected by the config file, then exit.
    /// Pricing is included where known.
    #[arg(long)]
    pub model_list: bool,

    /// Launch the configuration TUI to edit the config file interactively.
    #[arg(long)]
    pub init: bool,
}

impl Cli {
    /// Joined command-line input. When the positional args are empty the
    /// caller is expected to read from stdin (handled at a higher layer).
    pub fn input(&self) -> String {
        self.command.join(" ")
    }
}
