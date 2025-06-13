use crate::models::ImageMetadata;
use anyhow::Result;
use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, error, info};

/// Manages storage and retrieval of image metadata
#[derive(Clone)]
pub struct MetadataManager {
    project_dirs: ProjectDirs,
}

impl MetadataManager {
    /// Create a new MetadataManager instance
    pub fn new() -> Result<Self> {
        let project_dirs = ProjectDirs::from("network", "Golem Factory", "GPU Imager")
            .ok_or_else(|| anyhow::anyhow!("Failed to get project directories"))?;

        // Ensure data directory exists
        let data_dir = project_dirs.data_dir();
        if !data_dir.exists() {
            fs::create_dir_all(data_dir)?;
            info!("Created metadata data directory: {:?}", data_dir);
        }

        Ok(Self { project_dirs })
    }

    /// Get the path for storing metadata for a given compressed image hash
    fn get_metadata_path(&self, compressed_hash: &str) -> PathBuf {
        self.project_dirs
            .data_dir()
            .join(format!("{}.metadata.json", compressed_hash))
    }

    /// Store metadata for an image
    pub fn store_metadata(&self, compressed_hash: &str, metadata: &ImageMetadata) -> Result<()> {
        let metadata_path = self.get_metadata_path(compressed_hash);

        debug!(
            "Storing metadata for hash {} at {:?}",
            compressed_hash, metadata_path
        );

        let metadata_json = serde_json::to_string_pretty(metadata)?;
        fs::write(&metadata_path, metadata_json)?;

        info!(
            "Successfully stored metadata for image hash: {}",
            compressed_hash
        );
        Ok(())
    }

    /// Load metadata for an image if it exists
    pub fn load_metadata(&self, compressed_hash: &str) -> Result<Option<ImageMetadata>> {
        let metadata_path = self.get_metadata_path(compressed_hash);

        if !metadata_path.exists() {
            debug!("No metadata file found for hash: {}", compressed_hash);
            return Ok(None);
        }

        debug!(
            "Loading metadata for hash {} from {:?}",
            compressed_hash, metadata_path
        );

        let metadata_json = fs::read_to_string(&metadata_path)?;
        let metadata: ImageMetadata = serde_json::from_str(&metadata_json)?;

        debug!(
            "Successfully loaded metadata for image hash: {}",
            compressed_hash
        );
        Ok(Some(metadata))
    }

    /// Check if metadata exists for a given compressed hash
    #[allow(dead_code)]
    pub fn has_metadata(&self, compressed_hash: &str) -> bool {
        let metadata_path = self.get_metadata_path(compressed_hash);
        metadata_path.exists()
    }

    /// Delete metadata for an image
    #[allow(dead_code)]
    pub fn delete_metadata(&self, compressed_hash: &str) -> Result<()> {
        let metadata_path = self.get_metadata_path(compressed_hash);

        if metadata_path.exists() {
            fs::remove_file(&metadata_path)?;
            info!("Deleted metadata for image hash: {}", compressed_hash);
        } else {
            debug!("No metadata file to delete for hash: {}", compressed_hash);
        }

        Ok(())
    }

    /// List all images that have metadata stored
    #[allow(dead_code)]
    pub fn list_images_with_metadata(&self) -> Result<Vec<String>> {
        let data_dir = self.project_dirs.data_dir();
        let mut hashes = Vec::new();

        if !data_dir.exists() {
            return Ok(hashes);
        }

        for entry in fs::read_dir(data_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.ends_with(".metadata.json") {
                    if let Some(hash) = file_name.strip_suffix(".metadata.json") {
                        hashes.push(hash.to_string());
                    }
                }
            }
        }

        debug!("Found metadata for {} images", hashes.len());
        Ok(hashes)
    }

    /// Clean up orphaned metadata files (for images that no longer exist)
    #[allow(dead_code)]
    pub fn cleanup_orphaned_metadata<F>(&self, image_exists_fn: F) -> Result<usize>
    where
        F: Fn(&str) -> bool,
    {
        let hashes = self.list_images_with_metadata()?;
        let mut cleaned_count = 0;

        for hash in hashes {
            if !image_exists_fn(&hash) {
                match self.delete_metadata(&hash) {
                    Ok(_) => {
                        cleaned_count += 1;
                        info!("Cleaned up orphaned metadata for hash: {}", hash);
                    }
                    Err(e) => {
                        error!("Failed to clean up metadata for hash {}: {}", hash, e);
                    }
                }
            }
        }

        if cleaned_count > 0 {
            info!("Cleaned up {} orphaned metadata files", cleaned_count);
        }

        Ok(cleaned_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_metadata() -> ImageMetadata {
        ImageMetadata {
            compressed_hash: "test_compressed_hash".to_string(),
            uncompressed_hash: "test_uncompressed_hash".to_string(),
            uncompressed_size: 1024 * 1024 * 1024, // 1GB
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_metadata_storage_and_retrieval() {
        let temp_dir = TempDir::new().unwrap();
        let _project_dirs = ProjectDirs::from("test", "test", "test").unwrap();

        // Override data directory for testing
        // Note: This is a simplified test - in real usage we'd need to properly mock ProjectDirs

        let metadata = create_test_metadata();
        let hash = "test_hash";

        // Create a simple test by directly using the storage logic
        let data_dir = temp_dir.path().join("data");
        fs::create_dir_all(&data_dir).unwrap();

        let metadata_path = data_dir.join(format!("{}.metadata.json", hash));
        let metadata_json = serde_json::to_string_pretty(&metadata).unwrap();
        fs::write(&metadata_path, metadata_json).unwrap();

        // Test loading
        let loaded_json = fs::read_to_string(&metadata_path).unwrap();
        let loaded_metadata: ImageMetadata = serde_json::from_str(&loaded_json).unwrap();

        assert_eq!(loaded_metadata.compressed_hash, metadata.compressed_hash);
        assert_eq!(
            loaded_metadata.uncompressed_hash,
            metadata.uncompressed_hash
        );
        assert_eq!(
            loaded_metadata.uncompressed_size,
            metadata.uncompressed_size
        );
        assert_eq!(loaded_metadata.created_at, metadata.created_at);
    }
}
