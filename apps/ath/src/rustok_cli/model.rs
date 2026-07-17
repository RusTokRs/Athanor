use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Debug)]
pub(crate) enum Command {
    ArchitectureContext {
        intent: String,
        flags: ArchitectureContextFlags,
    },
    FfaAudit(ManualFlags),
    FbaAudit(ManualFlags),
    PageBuilderAudit(ManualFlags),
    FfaSurface {
        module: String,
        surface: String,
        flags: ManualFlags,
    },
    FfaViolations(ManualFlags),
    FbaModule {
        module: String,
        flags: ManualFlags,
    },
    FbaPort {
        module: String,
        port: String,
        flags: ManualFlags,
    },
    FbaDependencies(ManualFlags),
    FbaViolations(ManualFlags),
    PageBuilderProvider(ManualFlags),
    PageBuilderConsumer {
        module: String,
        flags: ManualFlags,
    },
    PageBuilderViolations(ManualFlags),
}

#[derive(Debug, Clone)]
pub(super) struct ArchitectureContextFlags {
    pub(super) path: PathBuf,
    pub(super) module: Option<String>,
    pub(super) max_modules: usize,
    pub(super) max_contracts: usize,
    pub(super) max_interactions: usize,
    pub(super) max_evidence: usize,
    pub(super) json: bool,
    pub(super) deadline_unix_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub(super) struct ManualFlags {
    pub(super) path: PathBuf,
    pub(super) json: bool,
    pub(super) max_nodes: usize,
    pub(super) max_edges: usize,
    pub(super) module: Option<String>,
    pub(super) surface: Option<String>,
    pub(super) deadline_unix_ms: Option<u64>,
}

pub(crate) fn parse(args: &[String]) -> Result<Option<Command>> {
    let command = match args {
        [first, second, third, intent, rest @ ..]
            if first == "rustok" && second == "architecture" && third == "context" =>
        {
            Some(Command::ArchitectureContext {
                intent: intent.clone(),
                flags: parse_architecture_context_flags(rest)?,
            })
        }
        [first, second, third, rest @ ..]
            if first == "rustok" && second == "ffa" && third == "audit" =>
        {
            Some(Command::FfaAudit(parse_manual_flags(rest, true)?))
        }
        [first, second, third, rest @ ..]
            if first == "rustok" && second == "fba" && third == "audit" =>
        {
            Some(Command::FbaAudit(parse_manual_flags(rest, true)?))
        }
        [first, second, third, rest @ ..]
            if first == "rustok" && second == "page-builder" && third == "audit" =>
        {
            Some(Command::PageBuilderAudit(parse_manual_flags(rest, true)?))
        }
        [first, second, third, module, surface, rest @ ..]
            if first == "graph" && second == "ffa" && third == "surface" =>
        {
            Some(Command::FfaSurface {
                module: module.clone(),
                surface: surface.clone(),
                flags: parse_manual_flags(rest, true)?,
            })
        }
        [first, second, third, rest @ ..]
            if first == "graph" && second == "ffa" && third == "violations" =>
        {
            Some(Command::FfaViolations(parse_manual_flags(rest, true)?))
        }
        [first, second, third, module, rest @ ..]
            if first == "graph" && second == "fba" && third == "module" =>
        {
            Some(Command::FbaModule {
                module: module.clone(),
                flags: parse_manual_flags(rest, true)?,
            })
        }
        [first, second, third, module, port, rest @ ..]
            if first == "graph" && second == "fba" && third == "port" =>
        {
            Some(Command::FbaPort {
                module: module.clone(),
                port: port.clone(),
                flags: parse_manual_flags(rest, true)?,
            })
        }
        [first, second, third, rest @ ..]
            if first == "graph" && second == "fba" && third == "dependencies" =>
        {
            Some(Command::FbaDependencies(parse_manual_flags(rest, true)?))
        }
        [first, second, third, rest @ ..]
            if first == "graph" && second == "fba" && third == "violations" =>
        {
            Some(Command::FbaViolations(parse_manual_flags(rest, true)?))
        }
        [first, second, third, rest @ ..]
            if first == "graph" && second == "page-builder" && third == "provider" =>
        {
            Some(Command::PageBuilderProvider(parse_manual_flags(rest, true)?))
        }
        [first, second, third, module, rest @ ..]
            if first == "graph" && second == "page-builder" && third == "consumer" =>
        {
            Some(Command::PageBuilderConsumer {
                module: module.clone(),
                flags: parse_manual_flags(rest, true)?,
            })
        }
        [first, second, third, rest @ ..]
            if first == "graph" && second == "page-builder" && third == "violations" =>
        {
            Some(Command::PageBuilderViolations(parse_manual_flags(rest, true)?))
        }
        _ => None,
    };
    Ok(command)
}

fn parse_architecture_context_flags(args: &[String]) -> Result<ArchitectureContextFlags> {
    let mut flags = ArchitectureContextFlags {
        path: PathBuf::from("."),
        module: None,
        max_modules: 6,
        max_contracts: 16,
        max_interactions: 16,
        max_evidence: 20,
        json: false,
        deadline_unix_ms: None,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                flags.json = true;
                index += 1;
            }
            "--path" => {
                flags.path = PathBuf::from(required_value(args, index, "--path")?);
                index += 2;
            }
            "--module" => {
                flags.module = Some(required_value(args, index, "--module")?.clone());
                index += 2;
            }
            "--max-modules" => {
                flags.max_modules = parse_positive_limit(
                    required_value(args, index, "--max-modules")?,
                    "--max-modules",
                )?;
                index += 2;
            }
            "--max-contracts" => {
                flags.max_contracts = parse_positive_limit(
                    required_value(args, index, "--max-contracts")?,
                    "--max-contracts",
                )?;
                index += 2;
            }
            "--max-interactions" => {
                flags.max_interactions = parse_positive_limit(
                    required_value(args, index, "--max-interactions")?,
                    "--max-interactions",
                )?;
                index += 2;
            }
            "--max-evidence" => {
                flags.max_evidence = parse_positive_limit(
                    required_value(args, index, "--max-evidence")?,
                    "--max-evidence",
                )?;
                index += 2;
            }
            "--deadline-unix-ms" => {
                flags.deadline_unix_ms = Some(parse_deadline(required_value(
                    args,
                    index,
                    "--deadline-unix-ms",
                )?)?);
                index += 2;
            }
            value => anyhow::bail!("unknown Rustok architecture context flag `{value}`"),
        }
    }
    Ok(flags)
}

fn parse_manual_flags(args: &[String], allow_positional_path: bool) -> Result<ManualFlags> {
    let mut flags = ManualFlags {
        path: PathBuf::from("."),
        json: false,
        max_nodes: 80,
        max_edges: 160,
        module: None,
        surface: None,
        deadline_unix_ms: None,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                flags.json = true;
                index += 1;
            }
            "--path" => {
                flags.path = PathBuf::from(required_value(args, index, "--path")?);
                index += 2;
            }
            "--max-nodes" => {
                flags.max_nodes = parse_positive_limit(
                    required_value(args, index, "--max-nodes")?,
                    "--max-nodes",
                )?;
                index += 2;
            }
            "--max-edges" => {
                flags.max_edges = parse_positive_limit(
                    required_value(args, index, "--max-edges")?,
                    "--max-edges",
                )?;
                index += 2;
            }
            "--module" => {
                flags.module = Some(required_value(args, index, "--module")?.clone());
                index += 2;
            }
            "--surface" => {
                flags.surface = Some(required_value(args, index, "--surface")?.clone());
                index += 2;
            }
            "--deadline-unix-ms" => {
                flags.deadline_unix_ms = Some(parse_deadline(required_value(
                    args,
                    index,
                    "--deadline-unix-ms",
                )?)?);
                index += 2;
            }
            value if allow_positional_path && !value.starts_with('-') => {
                flags.path = PathBuf::from(value);
                index += 1;
            }
            value => anyhow::bail!("unknown Rustok architecture flag `{value}`"),
        }
    }
    Ok(flags)
}

fn required_value<'a>(args: &'a [String], index: usize, label: &str) -> Result<&'a String> {
    args.get(index + 1)
        .ok_or_else(|| anyhow::anyhow!("{label} requires a value"))
}

fn parse_positive_limit(value: &str, label: &str) -> Result<usize> {
    let parsed = value
        .parse::<usize>()
        .with_context(|| format!("{label} must be a positive integer"))?;
    if parsed == 0 {
        anyhow::bail!("{label} must be greater than zero");
    }
    Ok(parsed)
}

fn parse_deadline(value: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .context("--deadline-unix-ms must be an unsigned integer")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_intercepts_rustok_audit_deadline() {
        let command = parse(&[
            "rustok".to_string(),
            "ffa".to_string(),
            "audit".to_string(),
            "repo".to_string(),
            "--deadline-unix-ms".to_string(),
            "42".to_string(),
        ])
        .unwrap()
        .expect("focused Rustok command");

        assert!(matches!(
            command,
            Command::FfaAudit(ManualFlags {
                deadline_unix_ms: Some(42),
                ..
            })
        ));
    }

    #[test]
    fn parser_intercepts_architecture_graph_filters() {
        let command = parse(&[
            "graph".to_string(),
            "ffa".to_string(),
            "violations".to_string(),
            "--module".to_string(),
            "blog".to_string(),
            "--surface".to_string(),
            "admin".to_string(),
            "--deadline-unix-ms".to_string(),
            "99".to_string(),
        ])
        .unwrap()
        .expect("focused Rustok graph command");

        assert!(matches!(
            command,
            Command::FfaViolations(ManualFlags {
                module: Some(module),
                surface: Some(surface),
                deadline_unix_ms: Some(99),
                ..
            }) if module == "blog" && surface == "admin"
        ));
    }

    #[test]
    fn parser_leaves_standard_graph_commands_to_standard_interceptor() {
        assert!(
            parse(&["graph".to_string(), "pagerank".to_string()])
                .unwrap()
                .is_none()
        );
    }
}
