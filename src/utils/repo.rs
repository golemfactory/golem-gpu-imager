use directories::ProjectDirs;
use futures_util::StreamExt;
use reqwest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Version {
    pub id: String,
    pub path: String,
    pub sha256: String,
    pub created: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Channel {
    pub name: String,
    pub versions: Vec<Version>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RepoMetadata {
    pub channels: Vec<Channel>,
}

#[derive(Debug, Clone)]
pub enum DownloadStatus {
    NotStarted,
    InProgress {
        progress: f32,
        bytes_downloaded: u64,
        total_bytes: u64,
    },
    Completed {
        path: PathBuf,
    },
    Failed {
        error: String,
    },
}

#[derive(Debug, Clone)]
pub struct DownloadImage {
    pub channel: String,
    pub version: Version,
    pub status: DownloadStatus,
}

pub struct ImageRepo {
    project_dirs: ProjectDirs,
    metadata: Option<RepoMetadata>,
    repo_url: String,
    downloads: Arc<Mutex<HashMap<String, DownloadStatus>>>,
}

impl ImageRepo {
    pub fn new() -> Self {
        let project_dirs =
            directories::ProjectDirs::from("network", "Golem Factory", "GPU Imager").unwrap();
        let repo_url =
            "https://repo-golem-gpu-live.s3.eu-central-1.amazonaws.com/images".to_string();

        Self {
            project_dirs,
            metadata: None,
            repo_url,
            downloads: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn fetch_metadata(&mut self) -> Result<&RepoMetadata, String> {
        let metadata_url = format!("{}/meta.json", self.repo_url);

        let response = reqwest::get(&metadata_url)
            .await
            .map_err(|e| format!("Failed to fetch repository metadata: {}", e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Failed to fetch metadata, status: {}",
                response.status()
            ));
        }

        let metadata: RepoMetadata = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse metadata: {}", e))?;

        self.metadata = Some(metadata);

        Ok(self.metadata.as_ref().unwrap())
    }

    pub fn get_newest_version_for_channel(&self, channel_name: &str) -> Option<Version> {
        self.metadata
            .as_ref()?
            .channels
            .iter()
            .find(|c| c.name == channel_name)?
            .versions
            .iter()
            .max_by(|a, b| a.created.cmp(&b.created))
            .cloned()
    }

    pub fn get_all_versions_for_channel(&self, channel_name: &str) -> Option<Vec<Version>> {
        self.metadata
            .as_ref()?
            .channels
            .iter()
            .find(|c| c.name == channel_name)
            .map(|c| c.versions.clone())
    }

    pub fn get_available_channels(&self) -> Vec<String> {
        match &self.metadata {
            Some(metadata) => metadata.channels.iter().map(|c| c.name.clone()).collect(),
            None => vec![],
        }
    }

    pub fn get_download_status(&self, version_id: &str) -> DownloadStatus {
        self.downloads
            .lock()
            .unwrap()
            .get(version_id)
            .cloned()
            .unwrap_or(DownloadStatus::NotStarted)
    }

    pub fn get_image_path(&self, version: &Version) -> PathBuf {
        let cache_dir = self.project_dirs.cache_dir().to_path_buf();
        cache_dir.join(&version.path)
    }

    pub fn is_image_downloaded(&self, version: &Version) -> bool {
        let path = self.get_image_path(version);
        path.exists() && self.verify_hash(&path, &version.sha256).is_ok()
    }

    fn verify_hash(&self, file_path: &Path, expected_hash: &str) -> Result<(), String> {
        let mut file = File::open(file_path)
            .map_err(|e| format!("Failed to open file for hash verification: {}", e))?;

        let mut hasher = Sha256::new();
        let mut buffer = [0; 8192];

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .map_err(|e| format!("Failed to read file during hash verification: {}", e))?;

            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);
        }

        let hash = hasher.finalize();
        let hash_hex = hex::encode(hash);

        if hash_hex == expected_hash.to_lowercase() {
            Ok(())
        } else {
            Err(format!(
                "Hash verification failed. Expected: {}, got: {}",
                expected_hash, hash_hex
            ))
        }
    }

    pub async fn start_download(
        &self,
        _channel_name: &str,
        version: Version,
    ) -> Result<mpsc::Receiver<DownloadStatus>, String> {
        // Create cache directory if it doesn't exist
        let cache_dir = self.project_dirs.cache_dir();
        fs::create_dir_all(cache_dir)
            .map_err(|e| format!("Failed to create cache directory: {}", e))?;

        let final_path = self.get_image_path(&version);

        // If already downloaded and verified, return completed status
        if final_path.exists() {
            if let Ok(()) = self.verify_hash(&final_path, &version.sha256) {
                let (tx, rx) = mpsc::channel(100);
                let status = DownloadStatus::Completed { path: final_path };
                self.downloads
                    .lock()
                    .unwrap()
                    .insert(version.id.clone(), status.clone());
                let _ = tx.try_send(status);
                return Ok(rx);
            }
        }

        // Use a temporary file during download
        let temp_path = cache_dir.join(format!("{}.download", version.path));

        // Set initial status
        let status = DownloadStatus::InProgress {
            progress: 0.0,
            bytes_downloaded: 0,
            total_bytes: 0,
        };
        self.downloads
            .lock()
            .unwrap()
            .insert(version.id.clone(), status.clone());

        // Create channel for sending progress updates
        let (tx, rx) = mpsc::channel(100);

        // Create a clone of necessary values for the download task
        let file_url = format!("{}/{}", self.repo_url, version.path);
        let version_id = version.id.clone();
        let expected_hash = version.sha256.clone();
        let downloads = self.downloads.clone();
        let temp_path_clone = temp_path.clone();
        let final_path_clone = final_path.clone();

        // Spawn download task
        tokio::spawn(async move {
            let download_result = async {
                // Make the request
                let response = match reqwest::get(&file_url).await {
                    Ok(resp) => resp,
                    Err(e) => return Err(format!("Failed to fetch file: {}", e)),
                };

                if !response.status().is_success() {
                    return Err(format!(
                        "Failed to download file, status: {}",
                        response.status()
                    ));
                }

                let total_size = response.content_length().unwrap_or(0);
                let mut downloaded = 0u64;
                let mut output_file = match tokio::fs::File::create(&temp_path).await {
                    Ok(file) => file,
                    Err(e) => return Err(format!("Failed to create output file: {}", e)),
                };
                let mut stream = response.bytes_stream();

                while let Some(item) = stream.next().await {
                    let chunk = match item {
                        Ok(chunk) => chunk,
                        Err(e) => return Err(format!("Failed to download chunk: {}", e)),
                    };

                    if let Err(e) = output_file.write_all(&chunk).await {
                        return Err(format!("Failed to write chunk: {}", e));
                    }

                    downloaded += chunk.len() as u64;
                    let progress = if total_size > 0 {
                        downloaded as f32 / total_size as f32
                    } else {
                        0.0
                    };

                    // Update status
                    let status = DownloadStatus::InProgress {
                        progress,
                        bytes_downloaded: downloaded,
                        total_bytes: total_size,
                    };

                    downloads
                        .lock()
                        .unwrap()
                        .insert(version_id.clone(), status.clone());
                    let _ = tx.send(status).await;
                }

                // Close the file
                if let Err(e) = output_file.flush().await {
                    return Err(format!("Failed to flush file: {}", e));
                }
                drop(output_file);

                // Verify the hash
                match tokio::task::spawn_blocking(move || {
                    // Use a standalone hash verification function since we can't directly reference self.verify_hash
                    let mut file = match File::open(&temp_path_clone) {
                        Ok(f) => f,
                        Err(e) => {
                            return Err(format!("Failed to open file for hash verification: {}", e));
                        }
                    };

                    let mut hasher = Sha256::new();
                    let mut buffer = [0; 8192];

                    loop {
                        let bytes_read = match file.read(&mut buffer) {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                return Err(format!(
                                    "Failed to read file during hash verification: {}",
                                    e
                                ));
                            }
                        };

                        if bytes_read == 0 {
                            break;
                        }

                        hasher.update(&buffer[..bytes_read]);
                    }

                    let hash = hasher.finalize();
                    let hash_hex = hex::encode(hash);

                    if hash_hex != expected_hash.to_lowercase() {
                        let _ = fs::remove_file(&temp_path_clone);
                        return Err(format!(
                            "Hash verification failed. Expected: {}, got: {}",
                            expected_hash, hash_hex
                        ));
                    }

                    // Move temporary file to final location
                    if let Err(e) = fs::rename(&temp_path_clone, &final_path_clone) {
                        let _ = fs::remove_file(&temp_path_clone);
                        return Err(format!("Failed to rename file: {}", e));
                    }

                    Ok(final_path_clone)
                })
                .await
                {
                    Ok(result) => result,
                    Err(e) => Err(format!("Task panicked: {}", e)),
                }
            }
            .await;

            // Update final status
            let final_status = match download_result {
                Ok(path) => DownloadStatus::Completed { path },
                Err(e) => DownloadStatus::Failed { error: e },
            };

            downloads
                .lock()
                .unwrap()
                .insert(version_id.clone(), final_status.clone());
            let _ = tx.send(final_status).await;
        });

        Ok(rx)
    }

    pub fn clean_cache(&self) -> Result<(), String> {
        let cache_dir = self.project_dirs.cache_dir();

        // Clean up partial downloads
        if let Ok(entries) = fs::read_dir(cache_dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_file() {
                        let file_name = entry.file_name().to_string_lossy().to_string();
                        if file_name.ends_with(".download") {
                            let _ = fs::remove_file(entry.path());
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn it_works() {
        let repo = super::ImageRepo::new();
        eprintln!("local={:?}", repo.project_dirs.state_dir());
        eprintln!("cache={:?}", repo.project_dirs.cache_dir());
        eprintln!("config={:?}", repo.project_dirs.config_dir());
    }

    #[test]
    fn test_fetch_metadata() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut repo = ImageRepo::new();
            let result = repo.fetch_metadata().await;
            assert!(result.is_ok());

            let metadata = result.unwrap();
            assert!(!metadata.channels.is_empty());

            // Print available channels
            for channel in &metadata.channels {
                println!("Channel: {}", channel.name);
                for version in &channel.versions {
                    println!("  Version: {} ({})", version.id, version.created);
                }
            }
        });
    }
}
