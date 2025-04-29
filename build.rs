use windows_exe_info::versioninfo::VersionInfo;
use windows_exe_info::{icon, manifest};

fn main() {
    icon::icon_ico("resources/icon.ico");
    manifest("resources/Golem-GPU-Imager.manifest");
    VersionInfo::from_cargo_env().link().unwrap();
}
