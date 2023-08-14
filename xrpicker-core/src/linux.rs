// Copyright 2022-2023, Collabora, Ltd.
// SPDX-License-Identifier: MIT OR Apache-2.0

use xdg::{BaseDirectories, BaseDirectoriesError};

use crate::{
    api_layer::BaseApiLayer,
    manifest::{GenericManifest, FILE_INDIRECTION_ARROW},
    path_simplifier::PathSimplifier,
    platform::{Platform, PlatformApiLayer, PlatformRuntime},
    runtime::BaseRuntime,
    ActiveState, Error, ManifestError, ACTIVE_RUNTIME_FILENAME, OPENXR, OPENXR_MAJOR_VERSION,
};
use itertools::Itertools;
use std::ffi::OsStr;
use std::{
    collections::HashSet,
    fs,
    iter::once,
    os::unix::{self, prelude::OsStrExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const ETC: &str = "/etc";

fn make_path_suffix() -> PathBuf {
    Path::new(OPENXR).join(OPENXR_MAJOR_VERSION.to_string())
}

fn make_sysconfdir(suffix: &Path) -> PathBuf {
    Path::new(ETC).join(suffix)
}

fn make_api_layer_suffix_implicit(suffix: &Path) -> PathBuf {
    suffix.join("/api_layers/implicit.d")
}

fn make_api_layer_suffix_explicit(suffix: &Path) -> PathBuf {
    suffix.join("/api_layers/explicit.d")
}

#[derive(Debug, PartialEq, Eq)]
pub struct LinuxRuntime {
    base: BaseRuntime,
    orig_path: PathBuf,
}

impl LinuxRuntime {
    fn new(orig_path: &Path, canonical_path: &Path) -> Result<Self, Error> {
        let base = BaseRuntime::new(canonical_path)?;

        Ok(LinuxRuntime {
            base,
            orig_path: orig_path.to_owned(),
        })
    }
}

impl PlatformRuntime for LinuxRuntime {
    fn make_active(&self) -> Result<(), Error> {
        fn convert_err(e: BaseDirectoriesError) -> Error {
            Error::SetActiveError(e.to_string())
        }
        let dirs = BaseDirectories::new().map_err(convert_err)?;
        let suffix = make_path_suffix();
        let path = dirs.place_config_file(suffix.join(ACTIVE_RUNTIME_FILENAME))?;

        // First move the old file out of the way, if any.
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let move_target =
            dirs.place_config_file(suffix.join(format!("old_active_runtime{}.json", timestamp)))?;

        match fs::rename(&path, &move_target) {
            Ok(_) => {
                // Only keep our renamed file if it wasn't a symlink
                if let Ok(m) = move_target.symlink_metadata() {
                    if m.is_symlink() && fs::remove_file(&move_target).is_err() {
                        // that's ok
                        eprintln!(
                            "Got an error trying to remove an apparently-symlink {}",
                            move_target.display()
                        )
                    }
                }
            }
            Err(e) => {
                // ignore and hope it meant there was just nothing to move
                eprintln!(
                    "Got an error trying to rename {} to {}: {}",
                    path.display(),
                    move_target.display(),
                    e
                );
            }
        }
        unix::fs::symlink(self.base.get_manifest_path(), &path)?;
        Ok(())
    }

    fn get_runtime_name(&self) -> String {
        self.base.get_runtime_name()
    }

    fn get_manifests(&self) -> Vec<&Path> {
        vec![self.base.get_manifest_path()]
    }

    fn get_libraries(&self) -> Vec<PathBuf> {
        let path = self.base.resolve_library_path();
        vec![path]
    }

    fn describe(&self) -> String {
        let description = self.base.describe_manifest(self.base.get_manifest_path());
        if self.orig_path != self.base.get_manifest_path() {
            format!(
                "{}{}{}",
                PathSimplifier::new().simplify(&self.orig_path).display(),
                FILE_INDIRECTION_ARROW,
                description
            )
        } else {
            description
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct LinuxApiLayer {
    base: BaseApiLayer,
    orig_path: PathBuf,
}

impl LinuxApiLayer {
    fn new(orig_path: &Path, canonical_path: &Path) -> Result<Self, Error> {
        let base = BaseApiLayer::new(canonical_path)?;

        Ok(LinuxApiLayer {
            base,
            orig_path: orig_path.to_owned(),
        })
    }
}

impl PlatformApiLayer for LinuxApiLayer {
    fn toggle_layer(&mut self) -> Result<(), Error> {
        // First get the original path of the current Api layer and flip the target path between disabled / enabled.
        let mut new_path = self.orig_path.clone();
        if let Some(extension) = new_path.extension() {
            if extension == "disabled" {
                new_path.set_extension("json");
            } else {
                new_path.set_extension("disabled");
            }
        }

        match fs::rename(&self.orig_path, &new_path) {
            Ok(_) => {
                // Only keep our renamed file if it wasn't a symlink
                if let Ok(m) = new_path.symlink_metadata() {
                    if m.is_symlink() && fs::remove_file(&new_path).is_err() {
                        // that's ok
                        eprintln!(
                            "Got an error trying to remove an apparently-symlink {}",
                            new_path.display()
                        )
                    }
                }
            }
            Err(e) => {
                // ignore and hope it meant there was just nothing to move
                eprintln!(
                    "Got an error trying to rename {} to {}: {}",
                    self.orig_path.display(),
                    new_path.display(),
                    e
                );
            }
        }

        // Set the original path to the new one.
        self.orig_path = new_path;
        Ok(())
    }

    fn get_layer_name(&self) -> String {
        self.base.get_api_layer_name()
    }

    fn get_manifests(&self) -> Vec<&Path> {
        vec![self.base.get_manifest_path()]
    }

    fn get_libraries(&self) -> Vec<PathBuf> {
        let path = self.base.resolve_library_path();
        vec![path]
    }

    fn describe(&self) -> String {
        let description = self.base.describe_manifest(self.base.get_manifest_path());
        if self.orig_path != self.base.get_manifest_path() {
            format!(
                "{}{}{}",
                PathSimplifier::new().simplify(&self.orig_path).display(),
                FILE_INDIRECTION_ARROW,
                description
            )
        } else {
            description
        }
    }

    fn is_active(&self) -> Result<ActiveState, Error> {
        if let Some(extension) = self.orig_path.extension() {
            if extension == "disabled" {
                Ok(ActiveState::NotActive)
            } else {
                Ok(ActiveState::ActiveIndependentApiLayer)
            }
        } else {
            Err(Error::GetActiveError(format!(
                "Api Layer: '{}', Path: '{}'",
                self.base.get_api_layer_name(),
                self.orig_path.display()
            )))
        }
    }
}

pub struct LinuxPlatform {
    path_suffix: PathBuf,
    implicit_layer_suffix: PathBuf,
    explicit_layer_suffix: PathBuf,
}

impl LinuxPlatform {
    fn new() -> Self {
        let path_suffix = make_path_suffix();
        let implicit_layer_suffix = make_api_layer_suffix_implicit(&path_suffix);
        let explicit_layer_suffix = make_api_layer_suffix_explicit(&path_suffix);
        Self {
            path_suffix,
            implicit_layer_suffix,
            explicit_layer_suffix,
        }
    }
}

fn is_active_runtime_name(p: &Path) -> bool {
    p.file_name().map(|s| s.as_bytes()) == Some(ACTIVE_RUNTIME_FILENAME.as_bytes())
}

fn find_potential_runtime_manifests_xdg(suffix: &Path) -> impl Iterator<Item = PathBuf> {
    let suffix = suffix.to_owned();
    BaseDirectories::new()
        .ok()
        .into_iter()
        .flat_map(move |xdg_dirs| xdg_dirs.list_config_files(&suffix))
        .filter(|p| !is_active_runtime_name(p))
}

fn find_potential_runtime_manifests_sysconfdir(suffix: &Path) -> impl Iterator<Item = PathBuf> {
    make_sysconfdir(suffix)
        .read_dir()
        .into_iter()
        .flatten()
        .filter_map(|r| r.ok())
        .filter(|entry| {
            // keep only files and symlinks
            entry
                .metadata()
                .map(|m| m.is_file() || m.is_symlink())
                .unwrap_or(false)
        })
        .map(|entry| entry.path())
        .filter(|p| !is_active_runtime_name(p))
}

fn find_potential_api_layer_manifests_xdg(suffix: &Path) -> impl Iterator<Item = PathBuf> {
    let suffix = suffix.to_owned();
    BaseDirectories::new()
        .ok()
        .into_iter()
        .flat_map(move |xdg_dirs| xdg_dirs.list_config_files(&suffix))
}

fn find_potential_api_layer_manifests_sysconfdir(suffix: &Path) -> impl Iterator<Item = PathBuf> {
    make_sysconfdir(suffix)
        .read_dir()
        .into_iter()
        .flatten()
        .filter_map(|r| r.ok())
        .filter(|entry| {
            // keep only files and symlinks
            entry.metadata().map(|m| m.is_file()).unwrap_or(false)
        })
        .map(|entry| entry.path())
}

pub struct LinuxActiveRuntimeData(Option<PathBuf>);

impl LinuxActiveRuntimeData {
    fn new() -> Self {
        LinuxActiveRuntimeData(possible_active_runtimes().next())
    }

    fn check_runtime(&self, runtime: &LinuxRuntime) -> ActiveState {
        if let Some(active_path) = &self.0 {
            if active_path == runtime.base.get_manifest_path() {
                return ActiveState::ActiveIndependentRuntime;
            }
        }
        ActiveState::NotActive
    }
}

fn possible_active_runtimes() -> impl Iterator<Item = PathBuf> {
    let suffix = make_path_suffix().join(ACTIVE_RUNTIME_FILENAME);
    let etc_iter = once(make_sysconfdir(&suffix));
    // Warning: BaseDirectories returns increasing order of importance, which is
    // opposite of what we want, so we reverse it.
    let xdg_iter = BaseDirectories::new()
        .ok()
        .into_iter()
        .flat_map(move |d| d.find_config_files(&suffix))
        .rev();

    xdg_iter
        .chain(etc_iter)
        .filter(|p| {
            p.metadata()
                .map(|m| m.is_file() || m.is_symlink())
                .ok()
                .unwrap_or_default()
        })
        .filter_map(|p| p.canonicalize().ok())
}

fn possible_active_api_layers() -> impl Iterator<Item = PathBuf> {
    // let suffixes = make_api_layer_suffixes(&make_path_suffix())
    //     .iter_mut()
    //     .map(|p| p.join("*.json"))
    //     .collect_vec();
    // let mut out_paths: Vec<PathBuf> = Vec::new();
    //
    // for suffix in suffixes {
    //     let xgd = find_potential_api_layer_manifests_xdg(suffix.as_path());
    //     let etc = find_potential_api_layer_manifests_sysconfdir(suffix.as_path());
    //
    //     out_paths.append(&mut xgd.chain(etc).collect_vec());
    // }
    // out_paths.into_iter()

    //TODO: update this implementation to also use implicit layers later (Maybe)

    let suffix = make_api_layer_suffix_implicit(&make_path_suffix());
    let etc_iter = once(make_sysconfdir(&suffix));
    // Warning: BaseDirectories returns increasing order of importance, which is
    // opposite of what we want, so we reverse it.
    let xdg_iter = BaseDirectories::new()
        .ok()
        .into_iter()
        .flat_map(move |d| d.find_config_files(&suffix))
        .rev();

    xdg_iter
        .chain(etc_iter)
        .filter(|p| {
            p.metadata()
                .map(|m| m.is_file() || m.is_symlink())
                .ok()
                .unwrap_or_default()
        })
        .filter_map(|p| p.canonicalize().ok())
}

impl Platform for LinuxPlatform {
    type PlatformRuntimeType = LinuxRuntime;
    type PlatformActiveRuntimeData = LinuxActiveRuntimeData;
    type PlatformApiLayerType = LinuxApiLayer;

    fn find_available_runtimes(
        &self,
        extra_paths: Box<dyn '_ + Iterator<Item = PathBuf>>,
    ) -> Result<(Vec<Self::PlatformRuntimeType>, Vec<ManifestError>), Error> {
        let mut known_manifests: HashSet<PathBuf> = HashSet::default();

        let manifest_files = find_potential_runtime_manifests_xdg(&self.path_suffix)
            .chain(find_potential_runtime_manifests_sysconfdir(
                &self.path_suffix,
            ))
            .chain(possible_active_runtimes()) // put these almost last so they are only included if they mention a not-previously-found runtime
            .chain(extra_paths)
            .filter_map(|p| p.canonicalize().ok().map(|canonical| (p, canonical)));

        let mut runtimes = vec![];
        let mut nonfatal_errors = vec![];

        for (orig_path, canonical) in manifest_files {
            if known_manifests.contains(&orig_path) {
                continue;
            }
            if known_manifests.contains(&canonical) {
                continue;
            }
            let runtime = match LinuxRuntime::new(&orig_path, &canonical) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!(
                        "Error when trying to load {} -> {}: {}",
                        orig_path.display(),
                        canonical.display(),
                        e
                    );
                    nonfatal_errors.push(ManifestError(orig_path, e));
                    continue;
                }
            };
            runtimes.push(runtime);
            if orig_path != canonical {
                known_manifests.insert(canonical);
            }
            known_manifests.insert(orig_path);
        }
        Ok((runtimes, nonfatal_errors))
    }

    fn find_available_api_layers(
        &self,
        extra_paths: Box<dyn '_ + Iterator<Item = PathBuf>>,
    ) -> Result<(Vec<Self::PlatformApiLayerType>, Vec<ManifestError>), Error> {
        let mut known_manifests: HashSet<PathBuf> = HashSet::default();

        let manifest_files = find_potential_api_layer_manifests_xdg(&self.implicit_layer_suffix)
            .chain(find_potential_api_layer_manifests_sysconfdir(
                &self.implicit_layer_suffix,
            ))
            .chain(extra_paths)
            .filter_map(|p| p.canonicalize().ok().map(|canonical| (p, canonical)));

        let mut layers = vec![];
        let mut nonfatal_errors = vec![];

        for (orig_path, canonical) in manifest_files {
            // eprintln!("Manifest file: {}")

            if known_manifests.contains(&orig_path) {
                continue;
            }
            if known_manifests.contains(&canonical) {
                continue;
            }
            let layer = match LinuxApiLayer::new(&orig_path, &canonical) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!(
                        "Error when trying to load {} -> {}: {}",
                        orig_path.display(),
                        canonical.display(),
                        e
                    );
                    nonfatal_errors.push(ManifestError(orig_path, e));
                    continue;
                }
            };
            layers.push(layer);
            if orig_path != canonical {
                known_manifests.insert(canonical);
            }
            known_manifests.insert(orig_path);
        }
        Ok((layers, nonfatal_errors))
    }

    fn get_active_runtime_manifests(&self) -> Vec<PathBuf> {
        LinuxActiveRuntimeData::new().0.into_iter().collect()
    }

    fn get_active_api_layer_manifests(&self) -> Vec<PathBuf> {
        possible_active_api_layers()
            .filter(|p| p.extension().unwrap_or(OsStr::new("disabled")) != "disabled")
            .collect_vec()
    }

    fn get_active_runtime_data(&self) -> Self::PlatformActiveRuntimeData {
        LinuxActiveRuntimeData::new()
    }

    fn get_runtime_active_state(
        &self,
        runtime: &Self::PlatformRuntimeType,
        active_data: &Self::PlatformActiveRuntimeData,
    ) -> ActiveState {
        active_data.check_runtime(runtime)
    }
}

/// Call to create a platform-specific object implementing the `Platform` trait.
pub fn make_platform() -> LinuxPlatform {
    LinuxPlatform::new()
}
