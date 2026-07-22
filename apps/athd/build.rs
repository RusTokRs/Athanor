fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if target_os == "windows" && target_env == "msvc" {
        // Windows executables default to a much smaller main-thread stack than Linux.
        // athd's async command dispatcher and production daemon future exceed that
        // reserve before command execution, so match the 8 MiB stack commonly
        // available to the main thread on Linux.
        println!("cargo:rustc-link-arg-bin=athd=/STACK:8388608");
    }
}
