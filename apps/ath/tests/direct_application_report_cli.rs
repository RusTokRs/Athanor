use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(args)
        .output()
        .expect("run ath CLI")
}

#[test]
fn remaining_application_report_help_uses_direct_dispatcher() {
    for args in [
        ["api", "snapshot", "--help"],
        ["docs", "propose-fix", "--help"],
    ] {
        let output = run(&args);
        assert!(
            output.status.success(),
            "{} help failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("--json"));
        assert!(stdout.contains("--path"));
    }
}
