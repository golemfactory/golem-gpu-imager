use directories::ProjectDirs;
use iced::window::{Settings, icon};
use std::path::PathBuf;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, registry, util::SubscriberInitExt};

mod disk;
mod models;
mod style;
mod ui;
mod utils;
mod version;

pub fn main() -> iced::Result {
    // Initialize tracing with different default levels based on build profile
    let default_level = if cfg!(debug_assertions) {
        // In debug mode, show more detailed logs
        "debug,golem_gpu_imager=debug,iced_winit=error"
    } else {
        // In release mode, only show info and above
        "info,golem_gpu_imager=info,iced_winit=error"
    };

    // Allow overriding via environment variable
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    // Check if running from console
    let is_console = is_running_from_console();

    // Windows-specific console setup
    #[cfg(windows)]
    {
        if is_console {
            // Only enable ANSI support if running from a console
            enable_ansi_support();
        } else {
            // If not running from console, avoid allocating a console window
            // No action needed here, as we'll use the file logger instead
        }
    }

    // Set up logging based on whether we're in a console or not
    if is_console {
        // When running from a console, log to stdout
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_file(true)
            .with_line_number(true)
            .init();
    } else {
        // When not running from a console, log to file
        // Set up a rolling log file - daily rotation with a max of 5 files
        let log_dir = get_log_directory();
        let file_appender =
            RollingFileAppender::new(Rotation::DAILY, log_dir, "golem-gpu-imager.log");
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

        // We need to keep the guard alive for the duration of the program
        // So we'll store it in a static and intentionally leak it
        let _guard = Box::leak(Box::new(_guard));

        // Set up subscriber with file logging
        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false) // Disable ANSI colors in file
            .with_file(true)
            .with_line_number(true);

        registry().with(filter).with(file_layer).init();
    }

    tracing::info!(
        "Starting Golem GPU Imager v{} built {} (console mode: {})",
        version::VERSION,
        version::BUILD_TIME,
        is_console
    );

    // Check elevation status on Windows
    #[cfg(windows)]
    {
        let elevation_status = utils::get_elevation_status();
        tracing::info!("Privilege status: {}", elevation_status);

        if !utils::is_elevated() {
            tracing::warn!(
                "Application is not running with administrator privileges. Some operations may fail."
            );
            tracing::info!(
                "To run with administrator privileges, right-click the application and select 'Run as administrator'"
            );
        }
    }

    let mut settings = Settings::default();

    settings.icon = Some(icon::from_file_data(include_bytes!("./assets/icon.png"), None).unwrap());

    // Start the application and load repository data
    iced::application(
        ui::application::GolemGpuImager::new,
        ui::application::GolemGpuImager::update,
        ui::application::GolemGpuImager::view,
    )
    .title(ui::application::GolemGpuImager::title)
    .font(ui::ICON_FONT)
    .window(settings)
    .window_size(iced::Size::new(560f32 + 80f32, 720f32))
    .theme(|_| style::custom_theme())
    .centered()
    .run()
}

/// Check if the program is running in a console
fn is_running_from_console() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::{GetStdHandle, STD_OUTPUT_HANDLE};

        unsafe {
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            if handle == 0 {
                return false;
            }

            let mut mode: u32 = 0;
            // If GetConsoleMode succeeds, we're running from a console
            windows_sys::Win32::System::Console::GetConsoleMode(handle, &mut mode) != 0
        }
    }

    #[cfg(not(windows))]
    {
        // On Unix systems, check if stdout is a TTY
        use std::os::unix::io::AsRawFd;
        unsafe {
            let stdout_fd = std::io::stdout().as_raw_fd();
            libc::isatty(stdout_fd) != 0
        }
    }
}

/// Get the directory for log files
fn get_log_directory() -> PathBuf {
    // Use ProjectDirs to get platform-specific data directory
    // Match the convention used elsewhere in the codebase
    let project_dirs = ProjectDirs::from("network", "Golem Factory", "GPU Imager")
        .expect("Failed to determine project directory");

    // Use the data_local_dir (platform-specific)
    let mut log_dir = project_dirs.data_local_dir().to_path_buf();
    log_dir.push("logs");

    // Ensure the directory exists
    if !log_dir.exists() {
        let _ = std::fs::create_dir_all(&log_dir);
    }

    log_dir
}

#[cfg(windows)]
fn enable_ansi_support() {
    // Enable ANSI terminal processing on Windows
    use windows_sys::Win32::System::Console::{
        ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetStdHandle, STD_OUTPUT_HANDLE, SetConsoleMode,
    };

    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle == 0 {
            return;
        }

        let mut mode: u32 = 0;

        // Get current console mode
        if windows_sys::Win32::System::Console::GetConsoleMode(handle, &mut mode) == 0 {
            return;
        }

        // Enable ANSI processing
        mode |= ENABLE_VIRTUAL_TERMINAL_PROCESSING;

        // Set the new mode
        let _ = SetConsoleMode(handle, mode);
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use std::io::Read;
    use std::os::fd::FromRawFd;
    use udisks2::Client;
    use udisks2::zbus::zvariant::OwnedObjectPath;

    type DynError = Box<dyn std::error::Error>;

    async fn resovle_device(client: &Client, path: &str) -> Result<OwnedObjectPath, DynError> {
        let mut spec = HashMap::new();
        spec.insert("path", path.into());
        let mut obj = client
            .manager()
            .resolve_device(spec, HashMap::default())
            .await?;

        Ok(obj.pop().ok_or("no device found")?)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    #[ignore]
    async fn test_list() -> Result<(), DynError> {
        eprintln!("Hello, world!");
        let ds = rs_drivelist::drive_list()?;
        for drive in ds {
            eprintln!("{:?}", drive);
        }

        use libc::{O_CLOEXEC, O_EXCL, O_SYNC};
        use udisks2::*;

        let client = Client::new().await?;
        let drive_path = resovle_device(&client, "/dev/mmcblk0").await?;
        eprintln!("{:?}", drive_path);
        let block = client.object(drive_path)?.block().await?;
        //let block = client.block_for_drive(&drive, true).await.unwrap();
        let flags = O_EXCL | O_SYNC | O_CLOEXEC;
        let owned_fd = block
            .open_device(
                "r",
                [("flags", zbus::zvariant::Value::from(flags))]
                    .into_iter()
                    .collect(),
            )
            .await?;
        if let zbus::zvariant::Fd::Owned(owned_fd) = owned_fd.into() {
            let mut buf = [0u8; 1024];
            let mut file = std::fs::File::from(owned_fd);
            file.read_exact(&mut buf[..]).unwrap();
            eprintln!("{:?}", buf);
        }

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    #[ignore]
    async fn test_find() -> Result<(), DynError> {
        let client = Client::new().await?;
        let path = "/dev/sda";
        let mut spec = HashMap::new();
        spec.insert("path", path.into());
        let mut obj = client
            .manager()
            .resolve_device(spec, HashMap::default())
            .await?;

        let drive_path = client
            .object(obj.pop().unwrap())?
            .block()
            .await?
            .drive()
            .await?;
        let mut spec = HashMap::new();
        eprintln!("{:?}", drive_path);
        spec.insert("drive", drive_path.into());
        let obj = client
            .manager()
            .resolve_device(spec, HashMap::default())
            .await?;

        for obj_path in obj {
            eprintln!("{:?}", obj_path);
        }
        Ok(())
    }
}
