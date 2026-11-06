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
    let status = Command::new(env::var("ZIG").unwrap_or_else(|_| "zig".into()))
        .current_dir(root.join("src/interface/ffi"))
        .args(["build", "-Doptimize=ReleaseSafe", "--cache-dir"])
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
