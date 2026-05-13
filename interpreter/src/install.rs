//! `--install`: drop the shell integration for the current shell into the
//! config directory, run the configuration TUI, then print the snippet the
//! user should add to their shell rc file plus a short usage hint.
//!
//! The three shell files are embedded at compile time so the installed binary
//! is self-contained — it doesn't depend on the repo layout being present at
//! runtime.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::config::Config;
use crate::init_tui;
use crate::shell::ShellKind;

const ZSH_SCRIPT: &str = include_str!("../../shell/interpreter.zsh");
const BASH_SCRIPT: &str = include_str!("../../shell/interpreter.bash");
const FISH_SCRIPT: &str = include_str!("../../shell/interpreter.fish");

/// Default hotkey advertised in the post-install usage hint. Keep in sync
/// with the `INTERPRETER_KEY` default in each `shell/interpreter.*` file.
const DEFAULT_HOTKEY: &str = "Ctrl-G";

struct ShellInstall {
    /// Filename written to the config directory (e.g. `interpreter.zsh`).
    filename: &'static str,
    /// Embedded script body.
    script: &'static str,
    /// rc file under `$HOME` to source from (relative to home).
    rc_relative: &'static str,
    /// Pretty name for messages.
    pretty: &'static str,
}

fn install_for(kind: ShellKind) -> Result<ShellInstall> {
    match kind {
        ShellKind::Zsh => Ok(ShellInstall {
            filename: "interpreter.zsh",
            script: ZSH_SCRIPT,
            rc_relative: ".zshrc",
            pretty: "zsh",
        }),
        ShellKind::Bash => Ok(ShellInstall {
            filename: "interpreter.bash",
            script: BASH_SCRIPT,
            rc_relative: ".bashrc",
            pretty: "bash",
        }),
        ShellKind::Fish => Ok(ShellInstall {
            filename: "interpreter.fish",
            script: FISH_SCRIPT,
            rc_relative: ".config/fish/config.fish",
            pretty: "fish",
        }),
        ShellKind::Unknown => Err(anyhow!(
            "could not detect a supported shell from $SHELL — \
             expected one of bash, zsh, fish"
        )),
    }
}

pub fn run(config_path: PathBuf, config: Config) -> Result<()> {
    let kind = ShellKind::detect();
    let install = install_for(kind)?;

    let install_dir = config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("config path {} has no parent directory", config_path.display()))?;

    std::fs::create_dir_all(&install_dir)
        .with_context(|| format!("creating {}", install_dir.display()))?;

    let script_path = install_dir.join(install.filename);
    std::fs::write(&script_path, install.script)
        .with_context(|| format!("writing {}", script_path.display()))?;

    eprintln!(
        "interpreter: installed {} integration to {}",
        install.pretty,
        script_path.display()
    );
    eprintln!("interpreter: launching configuration TUI…");

    init_tui::run(config_path, config)?;

    print_post_install(&install, &script_path);
    Ok(())
}

fn print_post_install(install: &ShellInstall, script_path: &Path) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "$HOME".to_string());
    let rc_path = PathBuf::from(&home).join(install.rc_relative);
    let source_line = format!("source {}", script_path.display());

    println!();
    println!("Installation complete.");
    println!();
    println!(
        "To finish setup, add the following to {}:",
        rc_path.display()
    );
    println!();
    println!("    echo '{source_line}' >> {}", rc_path.display());
    println!();
    println!(
        "Then open a new {pretty} session (or `source {rc}`) and you're set.",
        pretty = install.pretty,
        rc = rc_path.display()
    );
    println!();
    println!("Usage:");
    println!(
        "  • Type a prompt or partial command at your shell prompt, e.g.\n\
         \n      list files including hidden\n"
    );
    println!(
        "  • Press {key} to send the current command line to `interpreter`.\n\
           The line is replaced in place with an executable shell command,\n\
           which you can review and then run with Enter.",
        key = DEFAULT_HOTKEY
    );
    println!();
    println!(
        "Override the hotkey by exporting INTERPRETER_KEY before sourcing\n\
         the integration file (e.g. INTERPRETER_KEY='^[i' for Alt-i in zsh)."
    );
}
