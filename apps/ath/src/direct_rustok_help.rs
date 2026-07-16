pub(crate) fn print_if_requested(args: &[String]) -> bool {
    if !args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return false;
    }
    let usage = match args {
        [first, second, third, ..]
            if first == "rustok" && second == "architecture" && third == "context" =>
        {
            "ath rustok architecture context <INTENT> [OPTIONS]"
        }
        [first, second, third, ..]
            if first == "rustok"
                && matches!(second.as_str(), "ffa" | "fba" | "page-builder")
                && third == "audit" =>
        {
            "ath rustok <ffa|fba|page-builder> audit [PATH] [OPTIONS]"
        }
        [first, second, third, ..]
            if first == "graph"
                && matches!(second.as_str(), "ffa" | "fba" | "page-builder")
                && matches!(
                    third.as_str(),
                    "surface"
                        | "violations"
                        | "module"
                        | "port"
                        | "dependencies"
                        | "provider"
                        | "consumer"
                ) =>
        {
            "ath graph <ffa|fba|page-builder> <COMMAND> [ARGS] [OPTIONS]"
        }
        _ => return false,
    };

    println!("Run a bounded RusTok read under the shared operation lifecycle");
    println!();
    println!("Usage: {usage}");
    println!();
    println!("Options:");
    println!("      --deadline-unix-ms <MS>  Absolute operation deadline in Unix milliseconds");
    println!("      --json                   Print the complete report as JSON");
    println!("      --path <PATH>            Project root where supported");
    println!("      --max-nodes <N>          Maximum graph nodes [default: 80]");
    println!("      --max-edges <N>          Maximum graph edges [default: 160]");
    println!("  -h, --help                   Print help");
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_standard_graph_help() {
        assert!(!print_if_requested(&[
            "graph".to_string(),
            "pagerank".to_string(),
            "--help".to_string(),
        ]));
    }
}
