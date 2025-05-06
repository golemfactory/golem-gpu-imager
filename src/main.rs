use iced::window::{Settings, icon};

mod models;
mod style;
mod ui;
mod utils;
mod version;

pub fn main() -> iced::Result {
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
    .window_size(iced::Size::new(560f32, 720f32))
    .theme(|_| style::custom_theme())
    .centered()
    .run()
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
}
