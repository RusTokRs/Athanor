use std::path::PathBuf;

use anyhow::Result;
use athanor_app::{
    ContextOptions, IndexOptions, InitOptions, context_project, index_project, init_project,
};
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
        /// Path to write successful validation-only result JSON.
        #[arg(long)]
        validation_result: Option<PathBuf>,
        /// Validate adapter contracts without writing snapshots, state, or read models.
        #[arg(long)]
        validate_only: bool,
    },
    /// Build a task-focused context pack from the latest canonical snapshot.
    Context {
        /// Task or question used to select relevant project knowledge.
        task: String,
        /// Project root. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Print the complete context pack as JSON.
        #[arg(long)]
        json: bool,
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
            validation_result,
            validate_only,
        }) => {
            let report = index_project(IndexOptions {
                root: path,
                validation_report,
                validation_result,
                validate_only,
            })
            .await?;
            if report.validate_only {
                println!(
                    "validated {} files against adapter contracts using snapshot {}",
                    report.files_indexed, report.snapshot
                );
                if let Some(validation_result) = &report.validation_result {
                    println!("wrote validation result to {}", validation_result.display());
                }
            } else {
                println!(
                    "indexed {} files into snapshot {}",
                    report.files_indexed, report.snapshot
                );
            }
            println!(
                "affected files: {} changed, {} unchanged, {} removed",
                report.changed_files, report.unchanged_files, report.removed_files
            );
            if !report.validate_only {
                println!("wrote JSONL to {}", report.output_dir.display());
            }
        }
        Some(Command::Context { task, path, json }) => {
            let pack = context_project(ContextOptions { root: path, task }).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&pack)?);
            } else {
                println!("{}", pack.summary);
                for file in &pack.files {
                    println!("file: {file}");
                }
                for scope in &pack.scope {
                    println!("entity: {scope}");
                }
                for diagnostic in &pack.diagnostics {
                    println!("diagnostic: {}", diagnostic.0);
                }
            }
        }
        None => {
            println!("Athanor {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}
