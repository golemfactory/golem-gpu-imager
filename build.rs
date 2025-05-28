use std::path::Path;
use windows_exe_info::versioninfo::VersionInfo;
use windows_exe_info::{icon, manifest};

fn main() {
    let base = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path: &Path = base.as_ref();
    let target = std::env::var("TARGET").unwrap();

    // Set Windows-specific build options
    if target.contains("windows") {
        // Set subsystem to GUI to avoid console allocation when run as a GUI application
        println!("cargo:rustc-link-arg=-Wl,--subsystem,windows");
        //println!("cargo:rustc-link-arg=/ENTRY:mainCRTStartup");
    }

    icon::icon_ico("resources/icon.ico");
    manifest("resources/Golem-GPU-Imager.manifest");
    VersionInfo::from_cargo_env().link().unwrap();
    println!("cargo::rerun-if-changed=build.rs");
}
