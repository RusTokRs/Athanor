use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::{IndexOptions, IndexReport, index_project};

pub const INDEX_BENCHMARK_SCHEMA: &str = "athanor.index_benchmark.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkSize {
    Small,
    Medium,
    Large,
}

impl BenchmarkSize {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    fn profile(self) -> FixtureProfile {
        match self {
            Self::Small => FixtureProfile {
                docs: 8,
                rust_modules: 8,
                openapi_specs: 1,
            },
            Self::Medium => FixtureProfile {
                docs: 48,
                rust_modules: 48,
                openapi_specs: 2,
            },
            Self::Large => FixtureProfile {
                docs: 160,
                rust_modules: 160,
                openapi_specs: 4,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct BenchmarkOptions {
    pub size: BenchmarkSize,
    pub root: Option<PathBuf>,
    pub keep_fixture: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkReport {
    pub schema: &'static str,
    pub size: BenchmarkSize,
    pub fixture_root: PathBuf,
    pub kept_fixture: bool,
    pub files_written: usize,
    pub total_ms: u64,
    pub index: IndexReport,
}

#[derive(Debug, Clone, Copy)]
struct FixtureProfile {
    docs: usize,
    rust_modules: usize,
    openapi_specs: usize,
}

pub async fn benchmark_index(options: BenchmarkOptions) -> Result<BenchmarkReport> {
    let started = Instant::now();
    let root = match options.root {
        Some(root) => root,
        None => temp_benchmark_root(options.size),
    };
    if root.exists() {
        fs::remove_dir_all(&root)
            .with_context(|| format!("failed to clear benchmark root {}", root.display()))?;
    }
    fs::create_dir_all(&root)
        .with_context(|| format!("failed to create benchmark root {}", root.display()))?;

    let files_written = write_fixture(&root, options.size.profile())?;
    let index = index_project(IndexOptions {
        root: root.clone(),
        validation_report: None,
        validation_result: None,
        validate_only: false,
    })
    .await
    .context("failed to benchmark index fixture")?;

    let kept_fixture = options.keep_fixture;
    if !options.keep_fixture {
        fs::remove_dir_all(&root)
            .with_context(|| format!("failed to remove benchmark root {}", root.display()))?;
    }

    Ok(BenchmarkReport {
        schema: INDEX_BENCHMARK_SCHEMA,
        size: options.size,
        fixture_root: root,
        kept_fixture,
        files_written,
        total_ms: elapsed_ms(started.elapsed()),
        index,
    })
}

fn write_fixture(root: &Path, profile: FixtureProfile) -> Result<usize> {
    fs::create_dir_all(root.join("docs"))
        .with_context(|| format!("failed to create {}", root.join("docs").display()))?;
    fs::create_dir_all(root.join("src"))
        .with_context(|| format!("failed to create {}", root.join("src").display()))?;
    fs::create_dir_all(root.join("api"))
        .with_context(|| format!("failed to create {}", root.join("api").display()))?;

    let mut files = 0;
    for index in 0..profile.docs {
        let endpoint = format!("/resource/{index}");
        fs::write(
            root.join("docs").join(format!("topic-{index:04}.md")),
            format!(
                "---\nid: doc://benchmark/topic-{index:04}\nentities:\n  - api://GET:{endpoint}\n---\n# Topic {index}\n\n## Purpose\n\nThis benchmark document links API and Rust surfaces.\n\n## Endpoint\n\nUses `{endpoint}` and `benchmark_module_{index}`.\n"
            ),
        )
        .with_context(|| format!("failed to write benchmark doc {index}"))?;
        files += 1;
    }

    let mut lib = String::new();
    for index in 0..profile.rust_modules {
        lib.push_str(&format!("pub mod benchmark_module_{index};\n"));
        fs::write(
            root.join("src")
                .join(format!("benchmark_module_{index}.rs")),
            format!(
                "pub struct Resource{index};\n\npub fn handle_resource_{index}() -> &'static str {{\n    \"resource-{index}\"\n}}\n"
            ),
        )
        .with_context(|| format!("failed to write benchmark Rust module {index}"))?;
        files += 1;
    }
    fs::write(root.join("src/lib.rs"), lib).context("failed to write benchmark lib.rs")?;
    files += 1;

    let paths_per_spec = profile.docs.div_ceil(profile.openapi_specs.max(1));
    for spec_index in 0..profile.openapi_specs {
        let first = spec_index * paths_per_spec;
        let last = ((spec_index + 1) * paths_per_spec).min(profile.docs);
        let mut yaml = format!(
            "openapi: 3.0.3\ninfo:\n  title: Benchmark {spec_index}\n  version: 1.0.0\npaths:\n"
        );
        for index in first..last {
            yaml.push_str(&format!(
                "  /resource/{index}:\n    get:\n      operationId: getResource{index}\n      responses:\n        '200':\n          description: ok\n"
            ));
        }
        fs::write(
            root.join("api").join(format!("openapi-{spec_index}.yaml")),
            yaml,
        )
        .with_context(|| format!("failed to write benchmark OpenAPI spec {spec_index}"))?;
        files += 1;
    }

    Ok(files)
}

fn temp_benchmark_root(size: BenchmarkSize) -> PathBuf {
    std::env::temp_dir().join(format!(
        "athanor-index-benchmark-{}-{}",
        size.as_str(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn elapsed_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn runs_small_index_benchmark() {
        let root = temp_benchmark_root(BenchmarkSize::Small);
        let report = benchmark_index(BenchmarkOptions {
            size: BenchmarkSize::Small,
            root: Some(root.clone()),
            keep_fixture: false,
        })
        .await
        .unwrap();

        assert_eq!(report.schema, INDEX_BENCHMARK_SCHEMA);
        assert_eq!(report.size, BenchmarkSize::Small);
        assert!(!report.kept_fixture);
        assert!(report.files_written > 0);
        assert_eq!(report.index.files_indexed, report.files_written);
        assert_eq!(
            report.index.metrics.pipeline.files_discovered,
            report.files_written
        );
        assert!(!root.exists());
    }
}
