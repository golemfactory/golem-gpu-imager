use std::path::PathBuf;
use directories;
use directories::ProjectDirs;

pub struct ImageRepo {
    project_dirs: ProjectDirs
}

impl ImageRepo {
    
    pub fn new() -> Self {
        let project_dirs =  directories::ProjectDirs::from("network", "Golem Factory", "GPU Imager").unwrap();
        
        Self {
            project_dirs
        }
    }
    
}


#[cfg(test)]
mod tests {
    
    #[test]
    fn it_works() {
        let repo = super::ImageRepo::new();
        eprintln!("local={:?}", repo.project_dirs.state_dir());
        eprintln!("cache={:?}", repo.project_dirs.cache_dir());
        eprintln!("config={:?}", repo.project_dirs.config_dir());
        
    }
}