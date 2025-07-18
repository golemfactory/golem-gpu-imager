use crate::models::CancelToken;
use crate::utils::streaming_hash_calculator::{ProcessingProgress, StreamingHashCalculator};
use directories::ProjectDirs;
use futures_util::StreamExt;
use iced::task;
use reqwest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
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
    Processing(ProcessingProgress),
    Completed {
        path: PathBuf,
        metadata: crate::models::ImageMetadata,
    },
    Failed {
        error: String,
    },
}

#[derive(Debug, Clone)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error(s)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error(e.to_string())
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error(e.to_string())
    }
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Error(e.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct DownloadImage {
    pub channel: String,
    pub version: Version,
    pub status: DownloadStatus,
}

pub struct ImageRepo {
    project_dirs: ProjectDirs,
    metadata: Arc<Mutex<Option<RepoMetadata>>>,
    repo_url: String,
    downloads: Arc<Mutex<HashMap<String, DownloadStatus>>>,
}

impl Default for ImageRepo {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageRepo {
    pub fn new() -> Self {
        let project_dirs =
            directories::ProjectDirs::from("network", "Golem Factory", "GPU Imager").unwrap();
        let repo_url =
            "https://repo-golem-gpu-live.s3.eu-central-1.amazonaws.com/images".to_string();

        Self {
            project_dirs,
            metadata: Arc::new(Mutex::new(None)),
            repo_url,
            downloads: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn fetch_metadata(&self) -> Result<RepoMetadata, String> {
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

        let mut metadata: RepoMetadata = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse metadata: {}", e))?;

        // In profile-susteen mode, filter channels to only show "susteen"
        #[cfg(feature = "profile-susteen")]
        {
            metadata.channels = metadata.channels
                .into_iter()
                .filter(|channel| channel.name == "susteen")
                .collect();
        }

        // Cache the metadata
        if let Ok(mut cached_metadata) = self.metadata.lock() {
            *cached_metadata = Some(metadata.clone());
        }

        Ok(metadata)
    }

    #[allow(dead_code)]
    pub fn get_newest_version_for_channel(&self, channel_name: &str) -> Option<Version> {
        let metadata = self.metadata.lock().ok()?;
        metadata
            .as_ref()?
            .channels
            .iter()
            .find(|c| c.name == channel_name)?
            .versions
            .iter()
            .max_by(|a, b| a.created.cmp(&b.created))
            .cloned()
    }

    #[allow(dead_code)]
    pub fn get_all_versions_for_channel(&self, channel_name: &str) -> Option<Vec<Version>> {
        let metadata = self.metadata.lock().ok()?;
        let metadata_ref = metadata.as_ref()?;
        metadata_ref
            .channels
            .iter()
            .find(|c| c.name == channel_name)
            .map(|c| c.versions.clone())
    }

    #[allow(dead_code)]
    pub fn get_available_channels(&self) -> Vec<String> {
        if let Ok(metadata) = self.metadata.lock() {
            if let Some(metadata_ref) = metadata.as_ref() {
                return metadata_ref
                    .channels
                    .iter()
                    .map(|c| c.name.clone())
                    .collect();
            }
        }
        vec![]
    }

    #[allow(dead_code)]
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
        path.exists()
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

    pub fn start_download(
        self: Arc<Self>,
        channel_name: &str,
        version: Version,
        cancel_token: CancelToken,
    ) -> impl task::Sipper<Result<(), Error>, DownloadStatus> + 'static {
        let this = self.clone();
        let _channel_name = channel_name.to_string();
        let version_id = version.id.clone();
        task::sipper(async move |mut sipper| -> Result<(), Error> {
            let this = this.clone();
            let repo_url = &this.repo_url;
            let file_url = format!("{}/{}", repo_url, version.path);
            let expected_hash = version.sha256.clone();
            let version_clone = version.clone();

            // Create cache directory if it doesn't exist
            let cache_dir = this.project_dirs.cache_dir();
            fs::create_dir_all(cache_dir)?;

            let final_path = {
                let cache_dir = this.project_dirs.cache_dir().to_path_buf();
                cache_dir.join(&version_clone.path)
            };

            // If already downloaded and verified, check if we have cached metadata
            if final_path.exists() {
                // Try to load existing metadata
                let metadata_manager = crate::utils::image_metadata::MetadataManager::new()
                    .map_err(|e| Error(format!("Failed to create metadata manager: {}", e)))?;

                if let Ok(Some(metadata)) = metadata_manager.load_metadata(&expected_hash) {
                    // Verify hash quickly
                    if this.verify_hash(&final_path, &expected_hash).is_ok() {
                        let status = DownloadStatus::Completed {
                            path: final_path.clone(),
                            metadata,
                        };
                        this.downloads
                            .lock()
                            .unwrap()
                            .insert(version_id.clone(), status.clone());
                        sipper.send(status).await;
                        return Ok(());
                    }
                }
                // If hash verification fails or no metadata, continue with download
            }

            // Use a temporary file during download
            let temp_path = cache_dir.join(format!("{}.download", version_clone.path));

            // Initialize streaming hash calculator
            let mut calculator = StreamingHashCalculator::new(cancel_token.clone());

            // Set initial download status
            let initial_progress = ProcessingProgress::new_download(0, 0);
            let status = DownloadStatus::Processing(initial_progress.clone());
            this.downloads
                .lock()
                .unwrap()
                .insert(version_id.clone(), status.clone());
            sipper.send(status).await;

            // Make the request
            let response = reqwest::get(&file_url).await?;

            if !response.status().is_success() {
                return Err(Error(format!(
                    "Failed to download file, status: {}",
                    response.status()
                )));
            }

            let total_size = response.content_length().unwrap_or(0);
            let mut downloaded = 0u64;
            let mut output_file = tokio::fs::File::create(&temp_path).await?;
            let mut stream = response.bytes_stream();

            // Download phase: stream chunks and calculate compressed hash
            while let Some(item) = stream.next().await {
                if cancel_token.is_cancelled() {
                    let _ = fs::remove_file(&temp_path);
                    return Err(Error("Download cancelled by user".to_string()));
                }

                let chunk = item?;

                // Process chunk for compressed hash calculation
                calculator.process_download_chunk(&chunk)?;

                // Write to file
                output_file.write_all(&chunk).await?;

                downloaded += chunk.len() as u64;

                // Send download progress
                let progress = ProcessingProgress::new_download(downloaded, total_size);
                let status = DownloadStatus::Processing(progress);
                this.downloads
                    .lock()
                    .unwrap()
                    .insert(version_id.clone(), status.clone());
                sipper.send(status).await;
            }

            // Close the file
            output_file.flush().await?;
            drop(output_file);

            // Verify compressed hash
            let compressed_hash = calculator.finalize_compressed_hash();
            if compressed_hash.to_lowercase() != expected_hash.to_lowercase() {
                let _ = fs::remove_file(&temp_path);
                return Err(Error(format!(
                    "Hash verification failed. Expected: {}, got: {}",
                    expected_hash, compressed_hash
                )));
            }

            // Move temporary file to final location
            if let Err(e) = fs::rename(&temp_path, &final_path) {
                let _ = fs::remove_file(&temp_path);
                return Err(e.into());
            }

            // Metadata calculation phase
            let final_path_clone = final_path.clone();
            let version_id_clone = version_id.clone();
            let this_clone = this.clone();
            let mut sipper_clone = sipper.clone();

            // Create a channel for progress updates
            let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();

            // Spawn task to handle progress updates
            let progress_handler = tokio::spawn(async move {
                while let Some(progress) = progress_rx.recv().await {
                    let status = DownloadStatus::Processing(progress);
                    this_clone
                        .downloads
                        .lock()
                        .unwrap()
                        .insert(version_id_clone.clone(), status.clone());
                    let _ = sipper_clone.send(status).await;
                }
            });

            let result = calculator
                .calculate_metadata(&final_path_clone, compressed_hash, progress_tx)
                .await;

            // Clean up progress handler
            progress_handler.abort();

            match result {
                Ok(metadata) => {
                    // Store metadata for future use
                    let metadata_manager = crate::utils::image_metadata::MetadataManager::new()
                        .map_err(|e| Error(format!("Failed to create metadata manager: {}", e)))?;

                    if let Err(e) = metadata_manager.store_metadata(&expected_hash, &metadata) {
                        tracing::warn!("Failed to store metadata: {}", e);
                    }

                    // Update final status to completed
                    let final_status = DownloadStatus::Completed {
                        path: final_path,
                        metadata,
                    };
                    this.downloads
                        .lock()
                        .unwrap()
                        .insert(version_id.clone(), final_status.clone());
                    sipper.send(final_status).await;
                    Ok(())
                }
                Err(e) => {
                    // Update final status to failed
                    let error_msg = e.to_string();
                    let final_status = DownloadStatus::Failed { error: error_msg };
                    this.downloads
                        .lock()
                        .unwrap()
                        .insert(version_id.clone(), final_status.clone());
                    sipper.send(final_status).await;
                    Err(Error(e.to_string()))
                }
            }
        })
    }

    #[allow(dead_code)]
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

    #[test]
    fn test_path() {
        let project_dirs = ProjectDirs::from("network", "Golem Factory", "GPU Imager").unwrap();
        eprintln!("Data dir: {:?}", project_dirs.data_dir());
        eprintln!("Cache dir: {:?}", project_dirs.cache_dir());
        eprintln!("Config dir: {:?}", project_dirs.config_dir());
    }
}
