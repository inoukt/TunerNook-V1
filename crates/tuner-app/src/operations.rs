use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{fs, io::Write as _};

use tuner_core::{sha256_hex, BinDocument};
use tuner_transfer::{TransferOptions, TransferPlan};
use tuner_xdf::{ValidationReport, XdfDocument};

use crate::{
    build_compare_map_data, search_map_candidates_with_progress_and_cancel, search_snapshot,
    CompareMapData, MapCandidate, MapSearchConfig, ProjectPreferences, SearchResult, SearchState,
};
use tuner_core::ByteRange;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationKind {
    LoadingBin,
    LoadingXdf,
    LoadingCompareBin,
    LoadingCompareXdf,
    BuildingCompareMap,
    BuildingTransferPlan,
    Validating,
    RestoringWorkspace,
    SavingBin,
    SavingXdf,
    ExportingDebugReport,
    Searching,
    SearchingMaps,
}

impl OperationKind {
    pub fn messages(self) -> &'static [&'static str] {
        match self {
            Self::LoadingBin => &[
                "Reading BIN…",
                "Checking BIN size…",
                "Preparing calibration bytes…",
            ],
            Self::LoadingXdf => &[
                "Interpreting XDF…",
                "Building category map…",
                "Indexing tables…",
            ],
            Self::LoadingCompareBin => &[
                "Reading compare BIN…",
                "Checking compare BIN size…",
                "Preparing source bytes…",
            ],
            Self::LoadingCompareXdf => &[
                "Interpreting compare XDF…",
                "Checking source mappings…",
                "Indexing compare tables…",
            ],
            Self::BuildingCompareMap => &[
                "Reading source map…",
                "Reading destination map…",
                "Building comparison…",
            ],
            Self::BuildingTransferPlan => &[
                "Checking transfer compatibility…",
                "Reviewing mapped ranges…",
                "Building dry-run plan…",
            ],
            Self::Validating => &[
                "Validating mappings…",
                "Checking mapped ranges…",
                "Finishing validation…",
            ],
            Self::RestoringWorkspace => &[
                "Restoring workspace…",
                "Restoring categories…",
                "Restoring table windows…",
            ],
            Self::SavingBin => &["Saving BIN…", "Publishing safe output…", "Finishing save…"],
            Self::SavingXdf => &[
                "Exporting XDF…",
                "Writing conversion overrides…",
                "Rechecking exported XDF…",
            ],
            Self::ExportingDebugReport => &[
                "Exporting debug report…",
                "Writing diagnostic snapshot…",
                "Finishing report…",
            ],
            Self::Searching => &["Searching…", "Reading mapped values…", "Ranking results…"],
            Self::SearchingMaps => &[
                "Scanning BIN regions…",
                "Scoring map-shaped data…",
                "Ranking candidates…",
            ],
        }
    }

    pub fn label(self) -> &'static str {
        self.messages()[0]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationInfo {
    pub id: u64,
    pub kind: OperationKind,
    pub subject: String,
    pub generation: u64,
    pub phase: String,
}

#[derive(Clone, Debug)]
pub enum OperationPayload {
    Bin(BinDocument),
    Xdf(XdfDocument),
    CompareBin {
        document: BinDocument,
        sha256: String,
    },
    CompareXdf(XdfDocument),
    CompareMap(CompareMapData),
    TransferPlan(TransferPlan),
    Validation(ValidationReport),
    Search(Vec<SearchResult>),
    MapCandidates {
        candidates: Vec<MapCandidate>,
        cancelled: bool,
    },
    Restored(ProjectPreferences),
    Written,
}

#[derive(Debug)]
pub struct OperationResult {
    pub id: u64,
    pub generation: u64,
    pub subject: String,
    pub payload: Result<OperationPayload, String>,
}

impl OperationResult {
    pub fn bin(id: u64, document: BinDocument) -> Self {
        Self {
            id,
            generation: 0,
            subject: String::new(),
            payload: Ok(OperationPayload::Bin(document)),
        }
    }

    pub fn xdf(id: u64, document: XdfDocument) -> Self {
        Self {
            id,
            generation: 0,
            subject: String::new(),
            payload: Ok(OperationPayload::Xdf(document)),
        }
    }

    pub fn compare_bin(id: u64, subject: impl Into<String>, document: BinDocument) -> Self {
        let sha256 = sha256_hex(document.bytes());
        Self {
            id,
            generation: 0,
            subject: subject.into(),
            payload: Ok(OperationPayload::CompareBin { document, sha256 }),
        }
    }

    pub fn compare_xdf(id: u64, subject: impl Into<String>, document: XdfDocument) -> Self {
        Self {
            id,
            generation: 0,
            subject: subject.into(),
            payload: Ok(OperationPayload::CompareXdf(document)),
        }
    }

    pub fn compare_map(id: u64, subject: impl Into<String>, data: CompareMapData) -> Self {
        Self {
            id,
            generation: 0,
            subject: subject.into(),
            payload: Ok(OperationPayload::CompareMap(data)),
        }
    }

    pub fn transfer_plan(id: u64, plan: TransferPlan) -> Self {
        Self {
            id,
            generation: 0,
            subject: plan.plan_id.clone(),
            payload: Ok(OperationPayload::TransferPlan(plan)),
        }
    }

    pub fn validation(id: u64, report: ValidationReport) -> Self {
        Self {
            id,
            generation: 0,
            subject: String::new(),
            payload: Ok(OperationPayload::Validation(report)),
        }
    }

    pub fn restored(id: u64, subject: impl Into<String>, preferences: ProjectPreferences) -> Self {
        Self {
            id,
            generation: 0,
            subject: subject.into(),
            payload: Ok(OperationPayload::Restored(preferences)),
        }
    }

    pub fn search(id: u64, generation: u64, results: Vec<SearchResult>) -> Self {
        Self {
            id,
            generation,
            subject: String::new(),
            payload: Ok(OperationPayload::Search(results)),
        }
    }

    pub fn map_candidates(
        id: u64,
        subject: impl Into<String>,
        candidates: Vec<MapCandidate>,
        cancelled: bool,
    ) -> Self {
        Self {
            id,
            generation: 0,
            subject: subject.into(),
            payload: Ok(OperationPayload::MapCandidates {
                candidates,
                cancelled,
            }),
        }
    }

    pub fn written(id: u64, subject: impl Into<String>) -> Self {
        Self {
            id,
            generation: 0,
            subject: subject.into(),
            payload: Ok(OperationPayload::Written),
        }
    }

    pub fn error(id: u64, subject: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id,
            generation: 0,
            subject: subject.into(),
            payload: Err(message.into()),
        }
    }
}

#[derive(Debug)]
pub struct OperationCoordinator {
    next_id: u64,
    active: Option<OperationInfo>,
    last_message: Option<String>,
    last_error: Option<String>,
    stale_result_count: usize,
    sender: Sender<OperationResult>,
    receiver: Receiver<OperationResult>,
    started_at: Option<Instant>,
    finished_at: Option<Instant>,
}

impl Default for OperationCoordinator {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            next_id: 0,
            active: None,
            last_message: None,
            last_error: None,
            stale_result_count: 0,
            sender,
            receiver,
            started_at: None,
            finished_at: None,
        }
    }
}

impl OperationCoordinator {
    pub fn begin(&mut self, kind: OperationKind, subject: impl Into<String>) -> u64 {
        self.begin_with_generation(kind, subject, 0)
    }

    pub fn begin_with_generation(
        &mut self,
        kind: OperationKind,
        subject: impl Into<String>,
        generation: u64,
    ) -> u64 {
        self.next_id = self.next_id.saturating_add(1);
        let id = self.next_id;
        self.active = Some(OperationInfo {
            id,
            kind,
            subject: subject.into(),
            generation,
            phase: kind.label().to_string(),
        });
        self.started_at = Some(Instant::now());
        self.last_error = None;
        self.finished_at = None;
        id
    }

    pub fn active(&self) -> Option<&OperationInfo> {
        self.active.as_ref()
    }

    pub fn is_busy(&self) -> bool {
        self.active.is_some()
    }

    pub fn accepts(&self, id: u64) -> bool {
        self.active.as_ref().is_some_and(|active| active.id == id)
    }

    pub fn update(&mut self, id: u64, phase: impl Into<String>) -> bool {
        let Some(active) = self.active.as_mut() else {
            self.stale_result_count = self.stale_result_count.saturating_add(1);
            return false;
        };
        if active.id != id {
            self.stale_result_count = self.stale_result_count.saturating_add(1);
            return false;
        }
        active.phase = phase.into();
        true
    }

    pub fn message_at(&self, elapsed: Duration) -> &str {
        let Some(active) = self.active.as_ref() else {
            return self.last_message.as_deref().unwrap_or("Working…");
        };
        let messages = active.kind.messages();
        let index = (elapsed.as_secs_f32() / 1.4).floor() as usize % messages.len();
        messages[index]
    }

    pub fn complete(&mut self, id: u64, message: impl Into<String>) -> bool {
        if !self.accepts(id) {
            self.stale_result_count = self.stale_result_count.saturating_add(1);
            return false;
        }
        self.last_message = Some(message.into());
        self.active = None;
        self.started_at = None;
        self.finished_at = Some(Instant::now());
        true
    }

    pub fn cancel(&mut self, id: u64, message: impl Into<String>) -> bool {
        if !self.accepts(id) {
            self.stale_result_count = self.stale_result_count.saturating_add(1);
            return false;
        }
        self.last_message = Some(message.into());
        self.active = None;
        self.started_at = None;
        self.finished_at = Some(Instant::now());
        true
    }

    pub fn fail(&mut self, id: u64, message: impl Into<String>) -> bool {
        if !self.accepts(id) {
            self.stale_result_count = self.stale_result_count.saturating_add(1);
            return false;
        }
        let message = message.into();
        self.last_error = Some(message.clone());
        self.last_message = Some(message);
        self.active = None;
        self.started_at = None;
        self.finished_at = Some(Instant::now());
        true
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn stale_result_count(&self) -> usize {
        self.stale_result_count
    }

    pub fn elapsed(&self) -> Option<Duration> {
        self.started_at.map(|started| started.elapsed())
    }

    pub fn recent_error(&self) -> Option<&str> {
        if self.active.is_some() || self.finished_at?.elapsed() > Duration::from_secs(5) {
            return None;
        }
        self.last_error.as_deref()
    }

    pub fn take_results(&mut self) -> Vec<OperationResult> {
        let mut results = Vec::new();
        while let Ok(result) = self.receiver.try_recv() {
            results.push(result);
        }
        results
    }

    pub fn sender(&self) -> Sender<OperationResult> {
        self.sender.clone()
    }

    pub fn spawn_load_bin(&self, id: u64, path: PathBuf) {
        let sender = self.sender();
        thread::spawn(move || {
            let mut result = match BinDocument::load(&path) {
                Ok(document) => OperationResult::bin(id, document),
                Err(error) => {
                    OperationResult::error(id, path.display().to_string(), error.to_string())
                }
            };
            if result.subject.is_empty() {
                result.subject = path.display().to_string();
            }
            let _ = sender.send(result);
        });
    }

    pub fn spawn_load_xdf(&self, id: u64, path: PathBuf) {
        let sender = self.sender();
        thread::spawn(move || {
            let mut result = match XdfDocument::load(&path) {
                Ok(document) => OperationResult::xdf(id, document),
                Err(error) => {
                    OperationResult::error(id, path.display().to_string(), error.to_string())
                }
            };
            if result.subject.is_empty() {
                result.subject = path.display().to_string();
            }
            let _ = sender.send(result);
        });
    }

    pub fn spawn_load_compare_bin(&self, id: u64, path: PathBuf) {
        let sender = self.sender();
        thread::spawn(move || {
            let result = match BinDocument::load(&path) {
                Ok(document) => {
                    OperationResult::compare_bin(id, path.display().to_string(), document)
                }
                Err(error) => {
                    OperationResult::error(id, path.display().to_string(), error.to_string())
                }
            };
            let _ = sender.send(result);
        });
    }

    pub fn spawn_load_compare_xdf(&self, id: u64, path: PathBuf) {
        let sender = self.sender();
        thread::spawn(move || {
            let result = match XdfDocument::load(&path) {
                Ok(document) => {
                    OperationResult::compare_xdf(id, path.display().to_string(), document)
                }
                Err(error) => {
                    OperationResult::error(id, path.display().to_string(), error.to_string())
                }
            };
            let _ = sender.send(result);
        });
    }

    pub fn spawn_compare_map(
        &self,
        id: u64,
        source_xdf: XdfDocument,
        destination_xdf: XdfDocument,
        source_bin: BinDocument,
        destination_bin: BinDocument,
        semantic_id: String,
    ) {
        let sender = self.sender();
        thread::spawn(move || {
            let result = match build_compare_map_data(
                &source_xdf,
                &destination_xdf,
                &source_bin,
                &destination_bin,
                &semantic_id,
            ) {
                Ok(data) => OperationResult::compare_map(id, semantic_id, data),
                Err(error) => OperationResult::error(id, semantic_id, error),
            };
            let _ = sender.send(result);
        });
    }

    pub fn spawn_transfer_plan(
        &self,
        id: u64,
        source_xdf: XdfDocument,
        destination_xdf: XdfDocument,
        source_bin: BinDocument,
        destination_bin: BinDocument,
        options: TransferOptions,
    ) {
        let sender = self.sender();
        thread::spawn(move || {
            let plan = TransferPlan::build(
                &source_xdf,
                &destination_xdf,
                &source_bin,
                &destination_bin,
                &options,
            );
            let _ = sender.send(OperationResult::transfer_plan(id, plan));
        });
    }

    pub fn spawn_validate(&self, id: u64, xdf: XdfDocument, bin_bytes: Vec<u8>) {
        let sender = self.sender();
        thread::spawn(move || {
            let report = xdf.validate_against(&bin_bytes);
            let _ = sender.send(OperationResult::validation(id, report));
        });
    }

    pub fn spawn_restore_workspace(
        &self,
        id: u64,
        settings_path: PathBuf,
        identity: String,
        persistence_enabled: bool,
    ) {
        let sender = self.sender();
        thread::spawn(move || {
            let preferences = if persistence_enabled {
                crate::load_project_preferences(
                    &crate::project_settings_path(&settings_path, &identity),
                    &identity,
                )
            } else {
                ProjectPreferences::for_identity(&identity)
            };
            let _ = sender.send(OperationResult::restored(id, identity, preferences));
        });
    }

    pub fn spawn_save_bin(&self, id: u64, document: BinDocument, path: PathBuf) {
        let sender = self.sender();
        thread::spawn(move || {
            let result = match document.save_as(&path) {
                Ok(()) => OperationResult::written(id, path.display().to_string()),
                Err(error) => {
                    OperationResult::error(id, path.display().to_string(), error.to_string())
                }
            };
            let _ = sender.send(result);
        });
    }

    pub fn spawn_save_xdf(
        &self,
        id: u64,
        document: XdfDocument,
        path: PathBuf,
        overwrite_confirmed: bool,
    ) {
        let sender = self.sender();
        thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                let text = document
                    .to_xdf_text()
                    .map_err(|error| format!("could not serialize XDF: {error}"))?;
                if path.exists() {
                    replace_existing_xdf(&path, text.as_bytes(), overwrite_confirmed)?;
                } else {
                    write_new_file(&path, text.as_bytes())?;
                    if let Err(error) = XdfDocument::load(&path) {
                        let _ = fs::remove_file(&path);
                        return Err(format!(
                            "exported XDF failed re-parse verification: {error}"
                        ));
                    }
                }
                Ok(())
            })();
            let result = match result {
                Ok(()) => OperationResult::written(id, path.display().to_string()),
                Err(error) => OperationResult::error(id, path.display().to_string(), error),
            };
            let _ = sender.send(result);
        });
    }

    pub fn spawn_write_text(&self, id: u64, path: PathBuf, text: String) {
        let sender = self.sender();
        thread::spawn(move || {
            let result = write_new_file(&path, text.as_bytes());
            let result = match result {
                Ok(()) => OperationResult::written(id, path.display().to_string()),
                Err(error) => OperationResult::error(id, path.display().to_string(), error),
            };
            let _ = sender.send(result);
        });
    }

    pub fn spawn_search(
        &self,
        id: u64,
        generation: u64,
        xdf: XdfDocument,
        bin_bytes: Option<Vec<u8>>,
        state: SearchState,
    ) {
        let sender = self.sender();
        thread::spawn(move || {
            let results = search_snapshot(&xdf, bin_bytes.as_deref(), &state);
            let _ = sender.send(OperationResult::search(id, generation, results));
        });
    }

    pub fn spawn_map_search(
        &self,
        id: u64,
        subject: String,
        bin_bytes: Vec<u8>,
        mapped_ranges: Vec<ByteRange>,
        config: MapSearchConfig,
        progress_percent: Arc<AtomicU8>,
        cancellation: Arc<AtomicBool>,
    ) {
        let sender = self.sender();
        thread::spawn(move || {
            let outcome = search_map_candidates_with_progress_and_cancel(
                &bin_bytes,
                &mapped_ranges,
                config,
                &cancellation,
                |percent| progress_percent.store(percent, Ordering::Release),
            );
            let _ = sender.send(OperationResult::map_candidates(
                id,
                subject,
                outcome.candidates,
                outcome.cancelled,
            ));
        });
    }
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path.exists() {
        return Err(format!("Refusing to overwrite '{}'.", path.display()));
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("could not create '{}': {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("could not write '{}': {error}", path.display()))?;
    Ok(())
}

fn replace_existing_xdf(path: &Path, bytes: &[u8], confirmed: bool) -> Result<(), String> {
    if !confirmed {
        return Err(format!("Refusing to overwrite '{}'.", path.display()));
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "could not inspect XDF destination '{}': {error}",
            path.display()
        )
    })?;
    if !metadata.file_type().is_file() {
        return Err(format!(
            "Refusing to replace non-file XDF destination '{}'.",
            path.display()
        ));
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("definition.xdf"))
        .to_string_lossy();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let suffix = format!("{}-{nonce}", std::process::id());
    let staged_path = parent.join(format!("{file_name}.tunernook-stage-{suffix}"));
    let backup_path = parent.join(format!("{file_name}.tunernook-backup-{suffix}"));

    let mut stage_created = false;
    let stage_result = (|| -> Result<(), String> {
        let mut staged = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged_path)
            .map_err(|error| format!("could not stage XDF replacement: {error}"))?;
        stage_created = true;
        staged
            .write_all(bytes)
            .and_then(|()| staged.sync_all())
            .map_err(|error| format!("could not write staged XDF: {error}"))?;
        drop(staged);
        XdfDocument::load(&staged_path)
            .map_err(|error| format!("staged XDF failed re-parse verification: {error}"))?;
        Ok(())
    })();
    if let Err(error) = stage_result {
        if stage_created {
            let _ = fs::remove_file(&staged_path);
        }
        return Err(error);
    }

    if let Err(error) = fs::rename(path, &backup_path) {
        let _ = fs::remove_file(&staged_path);
        return Err(format!(
            "could not preserve existing XDF '{}': {error}",
            path.display()
        ));
    }
    if let Err(error) = fs::rename(&staged_path, path) {
        let restored = fs::rename(&backup_path, path);
        let _ = fs::remove_file(&staged_path);
        return Err(match restored {
            Ok(()) => format!("could not install verified XDF replacement: {error}"),
            Err(restore_error) => format!(
                "could not install verified XDF replacement ({error}); original remains at '{}' because restore failed ({restore_error})",
                backup_path.display()
            ),
        });
    }
    let _ = fs::remove_file(backup_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn wait_for_result(coordinator: &mut OperationCoordinator) -> OperationResult {
        for _ in 0..100 {
            if let Some(result) = coordinator.take_results().into_iter().next() {
                return result;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("background operation did not finish");
    }

    #[test]
    fn coordinator_rejects_stale_results_and_reports_current_phase() {
        let mut coordinator = OperationCoordinator::default();
        let old = coordinator.begin(OperationKind::LoadingXdf, "old.xdf");
        let new = coordinator.begin(OperationKind::LoadingBin, "new.bin");
        assert!(!coordinator.accepts(old));
        assert!(coordinator.accepts(new));
        assert!(coordinator.update(new, "reading payload"));
        assert_eq!(coordinator.active().unwrap().phase, "reading payload");
        assert_eq!(
            coordinator.message_at(Duration::from_secs_f32(0.5)),
            "Reading BIN…"
        );
        assert_eq!(
            coordinator.message_at(Duration::from_secs_f32(1.5)),
            "Checking BIN size…"
        );
        coordinator.complete(new, "Loaded BIN");
        assert!(coordinator.active().is_none());
    }

    #[test]
    fn compare_bin_result_carries_the_loaded_source_sha256() {
        let result =
            OperationResult::compare_bin(1, "source.bin", BinDocument::from_bytes(vec![1, 2, 3]));

        match result.payload.unwrap() {
            OperationPayload::CompareBin { document, sha256 } => {
                assert_eq!(document.bytes(), [1, 2, 3]);
                assert_eq!(
                    sha256,
                    "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81"
                );
            }
            other => panic!("unexpected compare BIN payload: {other:?}"),
        }
    }

    #[test]
    fn save_and_report_workers_publish_written_results() {
        let root = std::env::temp_dir().join(format!(
            "tunernook-operations-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let bin_path = root.join("edited.bin");
        let report_path = root.join("report.txt");
        let mut coordinator = OperationCoordinator::default();

        let save = coordinator.begin(OperationKind::SavingBin, "edited.bin");
        coordinator.spawn_save_bin(
            save,
            BinDocument::from_bytes(vec![1, 2, 3]),
            bin_path.clone(),
        );
        let result = wait_for_result(&mut coordinator);
        assert_eq!(result.id, save);
        assert!(matches!(result.payload, Ok(OperationPayload::Written)));
        assert_eq!(std::fs::read(&bin_path).unwrap(), [1, 2, 3]);

        let report = coordinator.begin(OperationKind::ExportingDebugReport, "report.txt");
        coordinator.spawn_write_text(report, report_path.clone(), "debug report".to_string());
        let result = wait_for_result(&mut coordinator);
        assert_eq!(result.id, report);
        assert!(matches!(result.payload, Ok(OperationPayload::Written)));
        assert_eq!(
            std::fs::read_to_string(&report_path).unwrap(),
            "debug report"
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn xdf_save_worker_reparses_output_and_refuses_overwrite() {
        let root = std::env::temp_dir().join(format!(
            "tunernook-xdf-save-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("exported.xdf");
        let document = XdfDocument::parse(
            br#"<XDFFORMAT><XDFCONSTANT uniqueid="rpm"><title>RPM</title><EMBEDDEDDATA mmedaddress="0" mmedelementsizebits="8" /></XDFCONSTANT></XDFFORMAT>"#,
        )
        .unwrap();
        let mut coordinator = OperationCoordinator::default();

        let operation = coordinator.begin(OperationKind::SavingXdf, path.display().to_string());
        coordinator.spawn_save_xdf(operation, document.clone(), path.clone(), false);
        let result = wait_for_result(&mut coordinator);
        assert!(matches!(result.payload, Ok(OperationPayload::Written)));
        assert!(XdfDocument::load(&path).is_ok());

        let refusing = coordinator.begin(OperationKind::SavingXdf, path.display().to_string());
        coordinator.spawn_save_xdf(refusing, document.clone(), path.clone(), false);
        let result = wait_for_result(&mut coordinator);
        assert!(result
            .payload
            .err()
            .is_some_and(|message| message.contains("Refusing to overwrite")));
        assert_eq!(XdfDocument::load(&path).unwrap().parameters[0].title, "RPM");
        assert!(replace_existing_xdf(&path, b"not a valid XDF", true).is_err());
        assert_eq!(XdfDocument::load(&path).unwrap().parameters[0].title, "RPM");
        let directory = root.join("destination-directory.xdf");
        std::fs::create_dir(&directory).unwrap();
        assert!(replace_existing_xdf(&directory, b"not a file", true).is_err());
        assert!(directory.is_dir());

        let replacement = XdfDocument::parse(
            br#"<XDFFORMAT><XDFHEADER><deftitle>Replacement</deftitle></XDFHEADER><XDFCONSTANT uniqueid="replacement"><title>Replacement</title><EMBEDDEDDATA mmedaddress="1" mmedelementsizebits="8" /></XDFCONSTANT></XDFFORMAT>"#,
        )
        .unwrap();
        let confirmed = coordinator.begin(OperationKind::SavingXdf, path.display().to_string());
        coordinator.spawn_save_xdf(confirmed, replacement, path.clone(), true);
        let result = wait_for_result(&mut coordinator);
        assert!(matches!(result.payload, Ok(OperationPayload::Written)));
        assert_eq!(
            XdfDocument::load(&path).unwrap().parameters[0].title,
            "Replacement"
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn workspace_restore_worker_returns_identity_scoped_preferences() {
        let mut coordinator = OperationCoordinator::default();
        let operation = coordinator.begin(OperationKind::RestoringWorkspace, "project");
        coordinator.spawn_restore_workspace(
            operation,
            PathBuf::new(),
            "c:/tuning/project.bin".to_string(),
            false,
        );
        let result = wait_for_result(&mut coordinator);
        assert_eq!(result.id, operation);
        assert_eq!(result.subject, "c:/tuning/project.bin");
        match result.payload {
            Ok(OperationPayload::Restored(preferences)) => {
                assert_eq!(preferences.bin_identity, "c:/tuning/project.bin");
            }
            other => panic!("unexpected restore result: {other:?}"),
        }
    }

    #[test]
    fn failed_operation_exposes_a_short_lived_user_facing_error() {
        let mut coordinator = OperationCoordinator::default();
        let operation = coordinator.begin(OperationKind::LoadingBin, "missing.bin");
        coordinator.fail(operation, "file was not found");
        assert_eq!(coordinator.recent_error(), Some("file was not found"));
    }
}
