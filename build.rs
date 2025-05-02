use std::path::Path;
use windows_exe_info::versioninfo::VersionInfo;
use windows_exe_info::{icon, manifest};

fn main() {
    let base = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path : &Path = base.as_ref();
    
    icon::icon_ico("resources/icon.ico");
    manifest("resources/Golem-GPU-Imager.manifest");
    VersionInfo::from_cargo_env().link().unwrap();
}
