use std::path::PathBuf;

use anyhow::Result;
use athanor_app::{IndexOptions, InitOptions, index_project, init_project};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ath", version, about = "Athanor command line interface")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialize Athanor metadata in a project.
    Init {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Index project files and export JSONL read-models.
    Index {
        /// Project root. Defaults to the current directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Path to write adapter validation reports when indexing fails validation.
        #[arg(long)]
        validation_report: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Init { path }) => {
            let report = init_project(InitOptions { root: path })?;
            println!("initialized Athanor project at {}", report.root.display());

            for path in report.created {
                println!("created {}", path.display());
            }
        }
        Some(Command::Index {
            path,
            validation_report,
        }) => {
            let report = index_project(IndexOptions {
                root: path,
                validation_report,
            })
            .await?;
            println!(
                "indexed {} files into snapshot {}",
                report.files_indexed, report.snapshot
            );
            println!(
                "affected files: {} changed, {} unchanged, {} removed",
                report.changed_files, report.unchanged_files, report.removed_files
            );
            println!("wrote JSONL to {}", report.output_dir.display());
        }
        None => {
            println!("Athanor {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}
