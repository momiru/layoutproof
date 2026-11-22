use std::{env, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=RUSTC");

    let version = env::var_os("RUSTC")
        .and_then(|compiler| Command::new(compiler).arg("--version").output().ok())
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok());
    let version = version
        .as_deref()
        .map(|value| value.trim())
        .unwrap_or("unknown");
    println!("cargo:rustc-env=A02_RUSTC_VERSION={version}");
}
