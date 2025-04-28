fn main() {
    // Only run icon embedding on Windows
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("resources/icon.ico");
        res.set_language(0x0409); // US English
        
        // Set application metadata
        res.set("FileDescription", "Golem GPU Imager");
        res.set("ProductName", "Golem GPU Imager");
        res.set("OriginalFilename", "golem-gpu-imager.exe");
        res.set("LegalCopyright", "Copyright © 2025");
        
        // Compile and link the resource file
        if let Err(e) = res.compile() {
            eprintln!("Failed to compile Windows resources: {}", e);
        }
    }
}