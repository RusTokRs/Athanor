use anyhow::Result;
use athanor_app::{
    IndexGenerationCleanupOptions, RepairCanonicalLatestOptions, RepairInspectOptions,
    RepairRecoverIndexCleanupOptions, RepairRecoverIndexOptions,
};

use super::model::Command;
use super::render::{
    print_help, print_index_retention, print_inspect, print_recover_index,
    print_recover_index_cleanup, print_repair_latest,
};

pub(crate) async fn run(command: Command) -> Result<()> {
    match command {
        Command::Help(topic) => {
            print_help(topic);
            Ok(())
        }
        Command::Inspect { path, json } => {
            let report = athanor_app::inspect_repair(RepairInspectOptions { root: path })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_inspect(&report);
            }
            Ok(())
        }
        Command::IndexRetention {
            path,
            dry_run,
            keep,
            confirmation_token,
            json,
        } => {
            let report = athanor_app::cleanup_index_generations(IndexGenerationCleanupOptions {
                root: path,
                dry_run,
                keep,
                confirmation_token,
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_index_retention(&report);
            }
            Ok(())
        }
        Command::RecoverIndex {
            path,
            dry_run,
            json,
        } => {
            let composition = athanor_runtime_defaults::production();
            let report = athanor_app::recover_index_publication_with_composition(
                RepairRecoverIndexOptions {
                    root: path,
                    dry_run,
                },
                &composition,
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_recover_index(&report);
            }
            Ok(())
        }
        Command::RecoverIndexCleanup {
            path,
            dry_run,
            json,
        } => {
            let report = athanor_app::recover_index_cleanup(RepairRecoverIndexCleanupOptions {
                root: path,
                dry_run,
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_recover_index_cleanup(&report);
            }
            Ok(())
        }
        Command::RepairLatest {
            path,
            dry_run,
            snapshot,
            json,
        } => {
            let composition = athanor_runtime_defaults::production();
            let report = athanor_app::repair_canonical_latest_with_composition(
                RepairCanonicalLatestOptions {
                    root: path,
                    dry_run,
                    snapshot,
                },
                &composition,
            )
            .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_repair_latest(&report);
            }
            Ok(())
        }
    }
}
