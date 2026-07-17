use anyhow::Result;

pub(crate) enum Command {
    Handled,
    Plugin(crate::direct_plugin_cli::Command),
    ValidateChanged(crate::direct_validate_changed_cli::Command),
    Repair(crate::repair::Command),
    ApplicationReport(crate::direct_application_report_cli::Command),
    Generation(crate::direct_generation_cli::Command),
    Config(crate::direct_config_cli::Command),
    Check(crate::direct_check_cli::Command),
    Rustok(crate::rustok_cli::Command),
    Graph(crate::direct_graph_cli::Command),
    Context(crate::direct_context_cli::Command),
    Search(crate::direct_search_cli::Command),
    Read(crate::direct_read::Command),
    Legacy,
}

pub(crate) fn parse(args: &[String]) -> Result<Command> {
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
    if let Some(command) = crate::direct_application_report_cli::parse(args)? {
        return Ok(Command::ApplicationReport(command));
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
    Ok(Command::Legacy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_graph_precedes_legacy_fallback() {
        let command = parse(&["graph".to_string(), "pagerank".to_string()]).unwrap();
        assert!(matches!(command, Command::Graph(_)));
    }

    #[test]
    fn unknown_command_uses_legacy_model_until_final_migration() {
        let command = parse(&["index".to_string(), ".".to_string()]).unwrap();
        assert!(matches!(command, Command::Legacy));
    }
}
