use std::fs;
use windows_exe_info::versioninfo::VersionInfo;
use windows_exe_info::{icon, manifest};

fn main() {
    let target = std::env::var("TARGET").unwrap();
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();

    // Set build timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    // Format as ISO-like datetime
    let datetime = chrono::DateTime::from_timestamp(now as i64, 0)
        .unwrap()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string();
    
    println!("cargo:rustc-env=BUILD_TIME={}", datetime);

    // Generate manifest with current version
    let manifest_content = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity
    name="GolemFactory.GolemGPUImager" version="{}.0"
    processorArchitecture="amd64"
    type="win32" />

  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}}"/>
      <supportedOS Id="{{1f676c76-80fa-4089-9586-5383f602ee61}}"/>
      <supportedOS Id="{{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}}"/>
      <supportedOS Id="{{35138b9a-5d96-4fbd-8e2d-a2440225f93a}}"/>
    </application>
  </compatibility>

  <asmv3:application xmlns:asmv3="http://schemas.microsoft.com/asm.v3">
    <asmv3:windowsSettings xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">
      <dpiAware>true</dpiAware>
      <longPathAware>true</longPathAware>
    </asmv3:windowsSettings>
  </asmv3:application>

  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3"> <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>"#, version);

    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let manifest_path = std::path::Path::new(&out_dir).join("Golem-GPU-Imager.manifest");
    fs::write(&manifest_path, manifest_content).unwrap();

    // Set Windows-specific build options
    if target.contains("windows") {
        // Set subsystem to GUI to avoid console allocation when run as a GUI application
        println!("cargo:rustc-link-arg=-Wl,--subsystem,windows");
        println!("cargo::rerun-if-changed=resources/Golem-GPU-Imager.manifest");
        //println!("cargo:rustc-link-arg=/ENTRY:mainCRTStartup");
    }

    icon::icon_ico("resources/icon.ico");
    manifest(manifest_path.to_str().unwrap());
    VersionInfo::from_cargo_env().link().unwrap();
    println!("cargo::rerun-if-changed=build.rs");
}
