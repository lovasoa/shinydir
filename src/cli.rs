use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
pub struct CLI {
    /// Custom config file to use
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Check configured directories and report misplaced files
    Check {
        /// Parent directory. Leave blank to check all configured directories
        target: Option<PathBuf>,

        /// Print the list of misplaced files (one per line) without additional formatting
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        list: bool,
    },
    /// Automatically move misplaced files according to set rules
    AutoMove {
        /// Parent directory. Leave blank to check all configured directories
        target: Option<PathBuf>,

        /// Print files that would be affected without actually moving them
        #[arg(id = "dry", long, action = clap::ArgAction::SetTrue)]
        dry_run: bool,
    },
}
