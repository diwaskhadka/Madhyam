//! Version and display name come from the repository's shared files (docs/RELEASING.md):
//! VERSION and branding/brand.ini. libmpv is found with pkg-config.

use std::fs;

fn brand(ini: &str, key: &str) -> String {
    ini.lines()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim().to_string())
        .unwrap_or_default()
}

fn main() {
    println!("cargo:rerun-if-changed=../VERSION");
    println!("cargo:rerun-if-changed=../branding/brand.ini");
    let version = fs::read_to_string("../VERSION")
        .expect("VERSION file")
        .trim()
        .to_string();
    let ini = fs::read_to_string("../branding/brand.ini").expect("branding/brand.ini");
    println!("cargo:rustc-env=MADHYAM_VERSION={version}");
    println!("cargo:rustc-env=MADHYAM_NAME={}", brand(&ini, "name"));
    println!(
        "cargo:rustc-env=MADHYAM_NATIVE_NAME={}",
        brand(&ini, "native_name")
    );
    pkg_config::Config::new()
        .atleast_version("1.109") // libmpv client API 1.109 = mpv 0.33, render API with advanced control
        .probe("mpv")
        .expect("libmpv development files (Debian/Ubuntu: libmpv-dev)");
}
