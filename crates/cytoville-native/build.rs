// SPDX-License-Identifier: AGPL-3.0-or-later
use std::{env, path::PathBuf, process::Command};
fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    for path in [
        "src/interface/ffi",
        "src/interface/generated",
        "external/evil-weevil/src/core",
        "external/evil-weevil/src/interface",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(path).display());
    }
    println!("cargo:rerun-if-env-changed=ZIG");
    if !root
        .join("external/evil-weevil/src/core/kernel.zig")
        .exists()
    {
        panic!("Initialise the pinned kernel: git submodule update --init --recursive");
    }
    let mut command = Command::new(env::var("ZIG").unwrap_or_else(|_| "zig".into()));
    command
        .current_dir(root.join("src/interface/ffi"))
        .args(["build", "-Doptimize=ReleaseSafe"]);
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let arch = env::var("CARGO_CFG_TARGET_ARCH").expect("Cargo target architecture");
        command.arg(format!("-Dtarget={arch}-windows-msvc"));
        // Rust supplies compiler_builtins to the final executable. Zig 0.16's
        // bundled compiler_rt COFF object is rejected by Microsoft's linker.
        command.arg("-Dbundle-compiler-rt=false");
    }
    let status = command
        .arg("--cache-dir")
        .arg(out.join("zig-cache"))
        .arg("--prefix")
        .arg(&out)
        .status()
        .expect("Zig 0.16.0 is required");
    assert!(status.success(), "Native kernel build failed");
    println!(
        "cargo:rustc-link-search=native={}",
        out.join("lib").display()
    );
    println!("cargo:rustc-link-lib=static=cytoville_native");
}
