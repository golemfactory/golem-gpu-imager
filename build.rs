use std::path::Path;
use windows_exe_info::versioninfo::VersionInfo;
use windows_exe_info::{icon, manifest};

fn main() {
    let base = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path: &Path = base.as_ref();

    // Set Windows-specific build options
    #[cfg(windows)]
    {
        // Set subsystem to GUI to avoid console allocation when run as a GUI application
        println!("cargo:rustc-link-arg=/SUBSYSTEM:WINDOWS");
        println!("cargo:rustc-link-arg=/ENTRY:mainCRTStartup");
    }
    
    icon::icon_ico("resources/icon.ico");
    manifest("resources/Golem-GPU-Imager.manifest");
    VersionInfo::from_cargo_env().link().unwrap();
}
