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

/// Bundle linux/icons/scalable/actions/*.svg into icons.gresource (registered in main.rs), so
/// the app never depends on what the system icon theme happens to contain.
fn compile_icons() {
    let dir = std::path::Path::new("icons/scalable/actions");
    println!("cargo:rerun-if-changed=icons/scalable/actions");
    let mut files: Vec<String> = fs::read_dir(dir)
        .expect("linux/icons/scalable/actions")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".svg"))
        .collect();
    files.sort();
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gresources>\n  <gresource prefix=\"/dev/madhyam/Madhyam/icons/scalable/actions\">\n",
    );
    for f in &files {
        xml.push_str(&format!("    <file>{f}</file>\n"));
    }
    xml.push_str("  </gresource>\n</gresources>\n");
    let xml_path = out.join("icons.gresource.xml");
    fs::write(&xml_path, xml).unwrap();
    let status = std::process::Command::new("glib-compile-resources")
        .arg(format!("--sourcedir={}", dir.display()))
        .arg(format!(
            "--target={}",
            out.join("icons.gresource").display()
        ))
        .arg(&xml_path)
        .status()
        .expect("glib-compile-resources (Debian/Ubuntu: libglib2.0-dev-bin)");
    assert!(status.success(), "glib-compile-resources failed");
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
    compile_icons();
    pkg_config::Config::new()
        .atleast_version("1.109") // libmpv client API 1.109 = mpv 0.33, render API with advanced control
        .probe("mpv")
        .expect("libmpv development files (Debian/Ubuntu: libmpv-dev)");
}
