// Publishes stripped PNGs with bounded retention and cancellation-safe file ownership.
use std::path::PathBuf;
use std::sync::Arc;

use crate::Snapshot;

pub(crate) struct ArtCache {
    directory: Arc<Directory>,
    files: Vec<ArtFile>,
    revision: u64,
}

impl ArtCache {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory: Arc::new(Directory(directory)), files: Vec::new(), revision: 0 }
    }

    pub async fn write(&mut self, snapshot: &Snapshot) -> Option<String> {
        let art = snapshot.artwork.clone()?;
        snapshot.metadata.as_ref()?;
        self.revision += 1;
        let directory = self.directory.clone();
        let path = directory.0.join(format!("{}.png", self.revision));
        let result =
            tokio::task::spawn_blocking(move || -> Result<ArtFile, Box<dyn std::error::Error + Send + Sync>> {
                let length = art.width.checked_mul(art.height).and_then(|pixels| pixels.checked_mul(4));
                if art.width == 0 || art.height == 0 || length.map(|length| length as usize) != Some(art.rgba.len()) {
                    return Err("Invalid artwork dimensions".into());
                }
                let mut file = ArtFile { path, directory, url: String::new() };
                std::fs::create_dir_all(&file.directory.0)?;
                image::save_buffer_with_format(
                    &file.path,
                    &art.rgba,
                    art.width,
                    art.height,
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )?;
                let absolute = std::fs::canonicalize(&file.path)?;
                file.url = url::Url::from_file_path(absolute).map_err(|_| "Invalid local artwork path")?.into();
                Ok(file)
            })
            .await;
        match result {
            Ok(Ok(file)) => {
                let url = file.url.clone();
                self.files.push(file);
                if self.files.len() > 2 {
                    self.files.remove(0);
                }
                Some(url)
            }
            Ok(Err(error)) => {
                tracing::warn!(error = error.as_ref() as &dyn std::error::Error, "MPRIS artwork unavailable");
                None
            }
            Err(error) => {
                tracing::warn!(error = &error as &dyn std::error::Error, "MPRIS artwork worker failed");
                None
            }
        }
    }
}

struct ArtFile {
    path: PathBuf,
    directory: Arc<Directory>,
    url: String,
}

impl Drop for ArtFile {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(error = &error as &dyn std::error::Error, "MPRIS artwork cleanup failed");
        }
    }
}

struct Directory(PathBuf);

impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir(&self.0)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(error = &error as &dyn std::error::Error, "MPRIS artwork directory cleanup failed");
        }
    }
}
