//! Shell Completion Command
//!
//! Generate shell completion scripts for bash, zsh, fish, etc.

use clap::{Args, CommandFactory};
use clap_complete::{Shell, generate};
use std::io::Write;
use thiserror::Error;

#[derive(Args, Debug)]
pub struct CompletionArgs {
    /// Shell type (bash, zsh, fish, elvish, powershell)
    #[arg(value_enum)]
    pub shell: Shell,

    /// Output file (default: stdout)
    #[arg(short, long)]
    pub output: Option<String>,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to generate completion: {0}")]
    GenerationFailed(String),

    #[error("IO error: {0}")]
    IoError(String),
}

pub fn execute(args: CompletionArgs) -> Result<(), Error> {
    let mut cmd = crate::Cli::command();
    let name = cmd.get_name().to_string();

    if let Some(output_path) = &args.output {
        let mut file =
            std::fs::File::create(output_path).map_err(|e| Error::IoError(e.to_string()))?;
        generate(args.shell, &mut cmd, &name, &mut file);
        file.flush().map_err(|e| Error::IoError(e.to_string()))?;
        println!("Completion script written to: {}", output_path);
    } else {
        generate(args.shell, &mut cmd, &name, &mut std::io::stdout());
    }

    Ok(())
}
