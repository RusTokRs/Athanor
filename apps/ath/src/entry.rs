use anyhow::{Context, Result};

mod repair_cli;

mod legacy {
    include!("main.rs");

    pub(crate) fn run() -> anyhow::Result<()> {
        main()
    }
}

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = repair_cli::parse(&args)? else {
        return legacy::run();
    };

    #[allow(deprecated)]
    {
        athanor_runtime_defaults::install();
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to start Athanor repair runtime")?;
    runtime.block_on(repair_cli::run(command))
}
