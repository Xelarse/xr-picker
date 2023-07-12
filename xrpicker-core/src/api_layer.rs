use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{manifest::ApiLayerManifest, manifest::GenericManifest, Error};

/// The path and parsed data of a api layer manifest.
///
/// Used inside platform-specific types that implement `PlatformRuntime`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BaseApiLayer {
    manifest_path: PathBuf,
    manifest: ApiLayerManifest,
}

impl BaseApiLayer {
    /// Create from a manifest path.
    ///
    /// Does not check whether the library is valid, just whether we can load and parse the JSON
    /// according to our schema.
    pub(crate) fn new(manifest_path: &Path) -> Result<Self, Error> {
        let contents = fs::read_to_string(manifest_path)?;
        let manifest: ApiLayerManifest = serde_json::from_str(&contents)?;
        if !manifest.is_file_format_version_ok() {
            return Err(Error::ManifestVersionMismatch);
        }
        Ok(BaseApiLayer {
            manifest_path: manifest_path.to_owned(),
            manifest,
        })
    }

    /// Get the path to our manifest
    pub(crate) fn get_manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    /// Get a name for the api layer, preferably the self-declared one.
    pub(crate) fn get_api_layer_name(&self) -> String {
        self.manifest.api_layer.name.clone()
    }

    /// Get the fully resolved, canonical path to the library in this manifest/layer, if possible
    pub(crate) fn resolve_library_path(&self) -> PathBuf {
        let notcanon = self
            .manifest_path
            .parent()
            .expect("files always have parents")
            .join(self.manifest.library_path());
        notcanon.canonicalize().unwrap_or(notcanon)
    }
}

impl GenericManifest for BaseApiLayer {
    fn library_path(&self) -> &str {
        self.manifest.library_path()
    }

    fn is_file_format_version_ok(&self) -> bool {
        self.manifest.is_file_format_version_ok()
    }
}
