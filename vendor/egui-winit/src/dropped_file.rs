use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) struct NativeFile {
    path: PathBuf,
}

impl From<PathBuf> for NativeFile {
    fn from(path: PathBuf) -> Self {
        Self { path }
    }
}

impl egui::DroppedFile for NativeFile {
    fn path(&self) -> &Path {
        &self.path
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.path).map_err(|err| err.to_string())
    }
    // Winit only supplies a native path here, never a browser File handle.
    // Browser image selection in hello-wgpu uses rfd::AsyncFileDialog instead.
    #[cfg(target_arch = "wasm32")]
    fn bytes_async(&self) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<Vec<u8>, String>> + '_>> {
        Box::pin(async { Err("Browser file contents require a browser File handle".into()) })
    }
}
