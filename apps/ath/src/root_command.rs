use anyhow::{Result, bail};

pub(crate) enum Command {
    Handled,
    Plugin(crate::direct_plugin_cli::Command),
    ValidateChanged(crate::direct_validate_changed_cli::Command),
    Repair(crate::repair::Command),
    Generation(crate::direct_generation_cli::Command),
    Config(crate::direct_config_cli::Command),
    Check(crate::direct_check_cli::Command),
    Rustok(crate::rustok_cli::Command),
    Graph(crate::direct_graph_cli::Command),
    Context(crate::direct_context_cli::Command),
    Search(crate::direct_search_cli::Command),
    Read(crate::direct_read::Command),
    Index(crate::index_cli::Command),
    Docs(crate::docs_cli::Command),
    Api(crate::api_cli::Command),
    Projects(crate::projects_cli::Command),
    Analysis(crate::analysis_cli::Command),
    Mcp(crate::mcp_cli::Command),
}

pub(crate) fn parse(args: &[String]) -> Result<Command> {
    if args.is_empty() {
        println!("Athanor {}", env!("CARGO_PKG_VERSION"));
        return Ok(Command::Handled);
    }
    if matches!(args, [flag] if flag == "--version" || flag == "-V") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(Command::Handled);
    }
    if matches!(args, [flag] if flag == "--help" || flag == "-h") {
        print_help();
        return Ok(Command::Handled);
    }
    if crate::direct_rustok_help::print_if_requested(args) {
        return Ok(Command::Handled);
    }
    if let Some(command) = crate::direct_plugin_cli::parse(args)? {
        return Ok(Command::Plugin(command));
    }
    if let Some(command) = crate::direct_validate_changed_cli::parse(args)? {
        return Ok(Command::ValidateChanged(command));
    }
    if let Some(command) = crate::repair::parse(args)? {
        return Ok(Command::Repair(command));
    }
    if let Some(command) = crate::direct_generation_cli::parse(args)? {
        return Ok(Command::Generation(command));
    }
    if let Some(command) = crate::direct_config_cli::parse(args)? {
        return Ok(Command::Config(command));
    }
    if let Some(command) = crate::direct_check_cli::parse(args)? {
        return Ok(Command::Check(command));
    }
    if let Some(command) = crate::rustok_cli::parse(args)? {
        return Ok(Command::Rustok(command));
    }
    if let Some(command) = crate::direct_graph_cli::parse(args)? {
        return Ok(Command::Graph(command));
    }
    if let Some(command) = crate::direct_context_cli::parse(args)? {
        return Ok(Command::Context(command));
    }
    if let Some(command) = crate::direct_search_cli::parse(args)? {
        return Ok(Command::Search(command));
    }
    if let Some(command) = crate::direct_read::parse(args)? {
        return Ok(Command::Read(command));
    }
    if let Some(command) = crate::index_cli::parse(args)? {
        return Ok(Command::Index(command));
    }
    if let Some(command) = crate::docs_cli::parse(args)? {
        return Ok(Command::Docs(command));
    }
    if let Some(command) = crate::api_cli::parse(args)? {
        return Ok(Command::Api(command));
    }
    if let Some(command) = crate::projects_cli::parse(args)? {
        return Ok(Command::Projects(command));
    }
    if let Some(command) = crate::analysis_cli::parse(args)? {
        return Ok(Command::Analysis(command));
    }
    if let Some(command) = crate::mcp_cli::parse(args)? {
        return Ok(Command::Mcp(command));
    }
    bail!(
        "unknown command `{}`; run `ath --help` for available commands",
        args[0]
    )
}

fn print_help() {
    println!("Athanor command line interface");
    println!();
    println!("Usage: ath <COMMAND> [OPTIONS]");
    println!();
    println!("Commands:");
    for (name, description) in [
        ("init", "Initialize Athanor metadata in a project"),
        ("index", "Index project files and export read models"),
        ("bench", "Run synthetic indexing benchmarks"),
        ("update", "Update the project index from changed files"),
        ("validate-changed", "Validate changed files without publishing"),
        ("context", "Build task-focused project context"),
        ("explain", "Explain one canonical entity"),
        ("overview", "Summarize the latest canonical snapshot"),
        ("impact", "Calculate change blast radius"),
        ("change-map", "Build a bounded change map"),
        ("check", "Inspect diagnostics"),
        ("docs", "Check and repair documentation metadata"),
        ("config", "Validate and inspect configuration"),
        ("api", "Snapshot and compare API contracts"),
        ("wiki", "Build the Markdown wiki"),
        ("report", "Build generated reports"),
        ("generate", "Publish an immutable generation"),
        ("graph", "Query and export the canonical graph"),
        ("projects", "Manage repository identities"),
        ("plugins", "Inspect and manage adapter plugins"),
        ("repair", "Inspect and repair local artifacts"),
        ("search", "Search the project knowledge base"),
        ("coverage", "Report bounded analysis coverage"),
        ("capabilities", "Report analysis completeness"),
        ("mcp", "Start the MCP stdio server"),
        ("rustok", "Inspect RusTok architecture contracts"),
    ] {
        println!("  {name:<18} {description}");
    }
    println!();
    println!("Options:");
    println!("  -h, --help       Print help");
    println!("  -V, --version    Print version");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_graph_uses_focused_model() {
        let command = parse(&["graph".to_string(), "pagerank".to_string()]).unwrap();
        assert!(matches!(command, Command::Graph(_)));
    }

    #[test]
    fn index_and_docs_use_root_owned_families() {
        assert!(matches!(
            parse(&["index".to_string(), ".".to_string()]).unwrap(),
            Command::Index(_)
        ));
        assert!(matches!(
            parse(&["docs".to_string(), "check".to_string()]).unwrap(),
            Command::Docs(_)
        ));
    }

    #[test]
    fn unknown_command_fails_without_legacy_fallback() {
        let error = parse(&["not-a-command".to_string()]).unwrap_err();
        assert!(error.to_string().contains("unknown command"));
    }
}
