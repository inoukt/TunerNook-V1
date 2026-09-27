use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{project_identity_for_path, ProjectPreferences};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TunerProjectFile {
    pub format_version: u32,
    pub bin_path: PathBuf,
    pub xdf_path: Option<PathBuf>,
    pub preferences: ProjectPreferences,
}

impl TunerProjectFile {
    pub fn from_json(source: &str) -> Result<Self, String> {
        let mut manifest: Self = serde_json::from_str(source).map_err(|error| error.to_string())?;
        manifest.synchronize_xdf_reference();
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format_version != FORMAT_VERSION {
            return Err(format!(
                "Unsupported TunerNook project version {}.",
                self.format_version
            ));
        }
        if self.bin_path.as_os_str().is_empty() {
            return Err("The project file does not contain a BIN path.".into());
        }
        let expected_identity = project_identity_for_path(&self.bin_path);
        if self.preferences.bin_identity != expected_identity {
            return Err("The project preferences do not match the referenced BIN path.".into());
        }
        Ok(())
    }

    pub fn to_json(&self) -> Result<String, String> {
        let mut manifest = self.clone();
        manifest.synchronize_xdf_reference();
        manifest.validate()?;
        serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())
    }

    fn synchronize_xdf_reference(&mut self) {
        self.preferences.last_xdf_path = self.xdf_path.clone();
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        project_identity_for_path, ProjectPreferences, WorkspaceSnapshot, WorkspaceViewState,
    };

    use super::TunerProjectFile;

    fn sample_manifest(xdf_path: Option<PathBuf>) -> TunerProjectFile {
        let bin_path = PathBuf::from("I:/calibration/engine.bin");
        let identity = project_identity_for_path(&bin_path);
        let mut preferences = ProjectPreferences::for_identity(&identity);
        preferences.active_workspace_id = 7;
        preferences.active_workspace_name = "Daily tuning".into();
        preferences.browser_filter = "torque*".into();
        preferences.last_xdf_path = xdf_path.clone();

        let mut named_workspace = ProjectPreferences::for_identity(&identity);
        named_workspace.active_workspace_id = 3;
        named_workspace.active_workspace_name = "Diagnostics".into();
        named_workspace.hex_window_open = true;
        preferences
            .saved_workspace_snapshots
            .push(WorkspaceSnapshot {
                id: 3,
                name: "Diagnostics".into(),
                state: WorkspaceViewState::capture(&named_workspace),
            });

        TunerProjectFile {
            format_version: 1,
            bin_path,
            xdf_path,
            preferences,
        }
    }

    #[test]
    fn manifest_round_trip_preserves_document_paths_and_workspace_snapshots() {
        let manifest = sample_manifest(Some(PathBuf::from("I:/calibration/engine.xdf")));

        let encoded = manifest.to_json().unwrap();
        let restored = TunerProjectFile::from_json(&encoded).unwrap();

        assert_eq!(
            restored.bin_path,
            PathBuf::from("I:/calibration/engine.bin")
        );
        assert_eq!(
            restored.xdf_path,
            Some(PathBuf::from("I:/calibration/engine.xdf"))
        );
        assert_eq!(restored.preferences, manifest.preferences);
        assert_eq!(restored.preferences.active_workspace_name, "Daily tuning");
        assert_eq!(restored.preferences.saved_workspace_snapshots.len(), 1);
        assert_eq!(
            restored.preferences.saved_workspace_snapshots[0]
                .state
                .hex_window_open,
            true
        );
    }

    #[test]
    fn manifest_accepts_bin_only_project() {
        let manifest = sample_manifest(None);

        assert_eq!(manifest.xdf_path, None);
        assert_eq!(manifest.preferences.last_xdf_path, None);
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn manifest_rejects_unsupported_version() {
        let mut manifest = sample_manifest(None);
        manifest.format_version = 99;

        assert!(manifest.validate().is_err());
    }

    #[test]
    fn manifest_rejects_empty_bin_path() {
        let mut manifest = sample_manifest(None);
        manifest.bin_path = PathBuf::new();

        assert!(manifest.validate().is_err());
    }

    #[test]
    fn manifest_rejects_preferences_for_a_different_bin_path() {
        let mut manifest = sample_manifest(None);
        manifest.preferences.bin_identity = "I:/calibration/other.bin".into();

        assert!(manifest.validate().is_err());
    }

    #[test]
    fn older_project_preferences_default_manifest_path_to_none() {
        let mut value = serde_json::to_value(ProjectPreferences::for_identity("bin-a")).unwrap();
        value.as_object_mut().unwrap().remove("project_file_path");

        let restored: ProjectPreferences = serde_json::from_value(value).unwrap();

        assert_eq!(restored.project_file_path, None);
    }
}
