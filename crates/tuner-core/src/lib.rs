//! Safety-critical, dependency-free primitives for TunerNook.
//!
//! The core deliberately owns all BIN mutations. Higher-level callers should use
//! [`BinDocument::transaction`] rather than editing byte buffers directly.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Versioned schema name for JSON Lines diagnostics.
pub const LOG_SCHEMA: &str = "tuner-log/v1";

/// Errors returned by the core layer.
#[derive(Debug)]
pub enum CoreError {
    Io {
        operation: &'static str,
        path: PathBuf,
        message: String,
    },
    InvalidArgument {
        name: String,
        value: String,
        message: String,
    },
    InvalidWidth {
        width: usize,
    },
    OutOfBounds {
        offset: usize,
        length: usize,
        size: usize,
    },
    ValueOutOfRange {
        value: i128,
        width: usize,
        signed: bool,
    },
    TransactionClosed,
    SaveTargetExists {
        path: PathBuf,
    },
    InvalidPath {
        operation: &'static str,
        path: PathBuf,
        message: String,
    },
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                message,
            } => write!(
                f,
                "I/O operation '{operation}' failed for '{}': {message}",
                path.display()
            ),
            Self::InvalidArgument {
                name,
                value,
                message,
            } => write!(f, "invalid argument {name}={value:?}: {message}"),
            Self::InvalidWidth { width } => {
                write!(
                    f,
                    "unsupported integer width {width}; expected 1, 2, 4, or 8 bytes"
                )
            }
            Self::OutOfBounds {
                offset,
                length,
                size,
            } => write!(
                f,
                "byte range offset={offset}, length={length} is outside BIN size {size}"
            ),
            Self::ValueOutOfRange {
                value,
                width,
                signed,
            } => write!(
                f,
                "value {value} cannot be represented as a {}integer of {width} bytes",
                if *signed { "signed " } else { "unsigned " }
            ),
            Self::TransactionClosed => write!(f, "transaction is already closed"),
            Self::SaveTargetExists { path } => write!(
                f,
                "refusing to overwrite existing output '{}'; choose a new path",
                path.display()
            ),
            Self::InvalidPath {
                operation,
                path,
                message,
            } => write!(
                f,
                "invalid path for {operation} ('{}'): {message}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for CoreError {}

fn io_error(operation: &'static str, path: &Path, error: std::io::Error) -> CoreError {
    CoreError::Io {
        operation,
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

fn checked_range(offset: usize, length: usize, size: usize) -> Result<usize, CoreError> {
    let end = offset.checked_add(length).ok_or(CoreError::OutOfBounds {
        offset,
        length,
        size,
    })?;
    if end > size {
        return Err(CoreError::OutOfBounds {
            offset,
            length,
            size,
        });
    }
    Ok(end)
}

fn validate_width(width: usize) -> Result<(), CoreError> {
    match width {
        1 | 2 | 4 | 8 => Ok(()),
        _ => Err(CoreError::InvalidWidth { width }),
    }
}

/// Byte order used by typed BIN access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    Little,
    Big,
}

/// A half-open byte range `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

impl ByteRange {
    pub fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
}

/// Summary of bytes changed by one committed transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditSummary {
    pub label: String,
    pub changed_bytes: usize,
    pub first_offset: Option<usize>,
    pub last_offset: Option<usize>,
}

impl EditSummary {
    fn from_changes(label: String, changes: &[ByteChange]) -> Self {
        Self {
            label,
            changed_bytes: changes.len(),
            first_offset: changes.first().map(|change| change.offset),
            last_offset: changes.last().map(|change| change.offset),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ByteChange {
    offset: usize,
    before: u8,
    after: u8,
}

#[derive(Debug, Clone)]
struct EditBatch {
    label: String,
    changes: Vec<ByteChange>,
}

/// A loaded BIN and its in-memory edit history.
#[derive(Debug, Clone)]
pub struct BinDocument {
    data: Vec<u8>,
    original: Vec<u8>,
    changed_bytes: usize,
    source: Option<PathBuf>,
    undo: Vec<EditBatch>,
    redo: Vec<EditBatch>,
}

impl BinDocument {
    /// Load a BIN without modifying the source file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        let path = path.as_ref();
        let data = fs::read(path).map_err(|error| io_error("read BIN", path, error))?;
        Ok(Self {
            original: data.clone(),
            data,
            changed_bytes: 0,
            source: Some(path.to_path_buf()),
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    /// Create an unsaved document from bytes.
    pub fn from_bytes(data: Vec<u8>) -> Self {
        Self {
            original: data.clone(),
            data,
            changed_bytes: 0,
            source: None,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn source_path(&self) -> Option<&Path> {
        self.source.as_deref()
    }

    pub fn is_dirty(&self) -> bool {
        self.changed_bytes != 0
    }

    pub fn changed_byte_count(&self) -> usize {
        self.changed_bytes
    }

    fn update_changed_byte_count(&mut self, offset: usize, before: u8, after: u8) {
        let original = self.original[offset];
        match (before == original, after == original) {
            (true, false) => self.changed_bytes += 1,
            (false, true) => self.changed_bytes -= 1,
            _ => {}
        }
    }

    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_depth(&self) -> usize {
        self.redo.len()
    }

    pub fn undo_history(&self) -> Vec<EditSummary> {
        self.undo
            .iter()
            .rev()
            .map(|batch| EditSummary::from_changes(batch.label.clone(), &batch.changes))
            .collect()
    }

    pub fn redo_history(&self) -> Vec<EditSummary> {
        self.redo
            .iter()
            .rev()
            .map(|batch| EditSummary::from_changes(batch.label.clone(), &batch.changes))
            .collect()
    }

    pub fn read_bytes(&self, offset: usize, length: usize) -> Result<&[u8], CoreError> {
        let end = checked_range(offset, length, self.data.len())?;
        Ok(&self.data[offset..end])
    }

    pub fn read_u8(&self, offset: usize) -> Result<u8, CoreError> {
        Ok(self.read_bytes(offset, 1)?[0])
    }

    pub fn read_uint(
        &self,
        offset: usize,
        width: usize,
        endianness: Endianness,
    ) -> Result<u64, CoreError> {
        validate_width(width)?;
        let bytes = self.read_bytes(offset, width)?;
        let mut value = 0u64;
        for (index, byte) in bytes.iter().copied().enumerate() {
            let shift = match endianness {
                Endianness::Little => index * 8,
                Endianness::Big => (width - 1 - index) * 8,
            };
            value |= u64::from(byte) << shift;
        }
        Ok(value)
    }

    pub fn read_int(
        &self,
        offset: usize,
        width: usize,
        endianness: Endianness,
    ) -> Result<i64, CoreError> {
        validate_width(width)?;
        let value = self.read_uint(offset, width, endianness)?;
        let bits = width * 8;
        if bits == 64 {
            return Ok(value as i64);
        }
        let sign_bit = 1u64 << (bits - 1);
        if value & sign_bit != 0 {
            Ok((value | (!0u64 << bits)) as i64)
        } else {
            Ok(value as i64)
        }
    }

    /// Start an atomic edit transaction. Dropping an uncommitted transaction restores
    /// every byte changed inside it.
    pub fn transaction(&mut self, label: impl Into<String>) -> Transaction<'_> {
        Transaction {
            document: self,
            label: label.into(),
            changes: BTreeMap::new(),
            finished: false,
        }
    }

    /// Undo the most recently committed transaction.
    pub fn undo(&mut self) -> Option<EditSummary> {
        let batch = self.undo.pop()?;
        for change in &batch.changes {
            self.update_changed_byte_count(change.offset, change.after, change.before);
            self.data[change.offset] = change.before;
        }
        let summary = EditSummary::from_changes(batch.label.clone(), &batch.changes);
        self.redo.push(batch);
        Some(summary)
    }

    /// Redo the most recently undone transaction.
    pub fn redo(&mut self) -> Option<EditSummary> {
        let batch = self.redo.pop()?;
        for change in &batch.changes {
            self.update_changed_byte_count(change.offset, change.before, change.after);
            self.data[change.offset] = change.after;
        }
        let summary = EditSummary::from_changes(batch.label.clone(), &batch.changes);
        self.undo.push(batch);
        Some(summary)
    }

    /// Write a new file without overwriting an existing path. The temporary file is
    /// created beside the target, fully synced, and published with a no-clobber
    /// hard-link operation so a concurrent writer cannot replace the output.
    pub fn save_as(&self, target: impl AsRef<Path>) -> Result<(), CoreError> {
        let target = target.as_ref();
        if target.as_os_str().is_empty() {
            return Err(CoreError::InvalidPath {
                operation: "save BIN",
                path: target.to_path_buf(),
                message: "output path is empty".to_string(),
            });
        }
        if target.exists() {
            return Err(CoreError::SaveTargetExists {
                path: target.to_path_buf(),
            });
        }
        let parent = target
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if !parent.exists() {
            return Err(CoreError::InvalidPath {
                operation: "save BIN",
                path: parent.to_path_buf(),
                message: "parent directory does not exist".to_string(),
            });
        }
        let file_name = target.file_name().ok_or_else(|| CoreError::InvalidPath {
            operation: "save BIN",
            path: target.to_path_buf(),
            message: "output path has no file name".to_string(),
        })?;
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let mut temporary = None;
        let mut file = None;
        for attempt in 0..32u32 {
            let candidate = parent.join(format!(
                ".{}.tuner-tmp-{}-{}",
                file_name.to_string_lossy(),
                std::process::id(),
                id + u64::from(attempt)
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(opened) => {
                    temporary = Some(candidate);
                    file = Some(opened);
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(io_error("create temporary BIN", &candidate, error)),
            }
        }
        let temporary = temporary.ok_or_else(|| CoreError::Io {
            operation: "create temporary BIN",
            path: target.to_path_buf(),
            message: "could not allocate a unique temporary output after 32 attempts".to_string(),
        })?;
        let mut file = match file {
            Some(file) => file,
            None => {
                let _ = fs::remove_file(&temporary);
                return Err(CoreError::Io {
                    operation: "create temporary BIN",
                    path: target.to_path_buf(),
                    message: "temporary output handle was not allocated".to_string(),
                });
            }
        };
        if let Err(error) = file.write_all(&self.data) {
            let _ = fs::remove_file(&temporary);
            return Err(io_error("write temporary BIN", &temporary, error));
        }
        if let Err(error) = file.sync_all() {
            let _ = fs::remove_file(&temporary);
            return Err(io_error("sync temporary BIN", &temporary, error));
        }
        drop(file);
        // Hard-linking the fully synced temporary file publishes the target with
        // create-new/no-clobber semantics on filesystems that support hard links.
        // Unlike rename, this cannot replace an output created by a concurrent actor.
        if let Err(error) = fs::hard_link(&temporary, target) {
            let _ = fs::remove_file(&temporary);
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                return Err(CoreError::SaveTargetExists {
                    path: target.to_path_buf(),
                });
            }
            return Err(io_error("atomically publish BIN", target, error));
        }
        if let Err(error) = fs::remove_file(&temporary) {
            return Err(io_error("remove temporary BIN", &temporary, error));
        }
        Ok(())
    }
}

/// A mutable transaction over a [`BinDocument`].
///
/// All writes are bounds checked. A transaction is atomic from the caller's point
/// of view: failed validation or dropping without `commit` restores its old bytes.
pub struct Transaction<'a> {
    document: &'a mut BinDocument,
    label: String,
    changes: BTreeMap<usize, ByteChange>,
    finished: bool,
}

impl<'a> Transaction<'a> {
    fn ensure_open(&self) -> Result<(), CoreError> {
        if self.finished {
            Err(CoreError::TransactionClosed)
        } else {
            Ok(())
        }
    }

    fn record(&mut self, offset: usize, after: u8) {
        let before_now = self.document.data[offset];
        if before_now == after {
            return;
        }
        if let Some(change) = self.changes.get_mut(&offset) {
            let revert = {
                change.after = after;
                change.before == change.after
            };
            self.document.data[offset] = after;
            if revert {
                self.changes.remove(&offset);
            }
            return;
        }
        self.changes.insert(
            offset,
            ByteChange {
                offset,
                before: before_now,
                after,
            },
        );
        self.document.data[offset] = after;
    }

    pub fn read_uint(
        &self,
        offset: usize,
        width: usize,
        endianness: Endianness,
    ) -> Result<u64, CoreError> {
        self.ensure_open()?;
        self.document.read_uint(offset, width, endianness)
    }

    pub fn read_int(
        &self,
        offset: usize,
        width: usize,
        endianness: Endianness,
    ) -> Result<i64, CoreError> {
        self.ensure_open()?;
        self.document.read_int(offset, width, endianness)
    }

    pub fn write_u8(&mut self, offset: usize, value: u8) -> Result<(), CoreError> {
        self.ensure_open()?;
        checked_range(offset, 1, self.document.data.len())?;
        self.record(offset, value);
        Ok(())
    }

    pub fn write_bytes(&mut self, offset: usize, bytes: &[u8]) -> Result<(), CoreError> {
        self.ensure_open()?;
        checked_range(offset, bytes.len(), self.document.data.len())?;
        for (index, value) in bytes.iter().copied().enumerate() {
            self.record(offset + index, value);
        }
        Ok(())
    }

    pub fn write_uint(
        &mut self,
        offset: usize,
        width: usize,
        endianness: Endianness,
        value: u64,
    ) -> Result<(), CoreError> {
        self.ensure_open()?;
        validate_width(width)?;
        if width < 8 {
            let max = (1u64 << (width * 8)) - 1;
            if value > max {
                return Err(CoreError::ValueOutOfRange {
                    value: i128::from(value),
                    width,
                    signed: false,
                });
            }
        }
        let bytes = encode_integer(width, endianness, value);
        self.write_bytes(offset, &bytes)
    }

    pub fn write_int(
        &mut self,
        offset: usize,
        width: usize,
        endianness: Endianness,
        value: i64,
    ) -> Result<(), CoreError> {
        self.ensure_open()?;
        validate_width(width)?;
        let bits = width * 8;
        if bits < 64 {
            let min = -(1i64 << (bits - 1));
            let max = (1i64 << (bits - 1)) - 1;
            if value < min || value > max {
                return Err(CoreError::ValueOutOfRange {
                    value: i128::from(value),
                    width,
                    signed: true,
                });
            }
        }
        let bytes = encode_integer(width, endianness, value as u64);
        self.write_bytes(offset, &bytes)
    }

    /// Commit all writes as one undoable history entry.
    pub fn commit(mut self) -> Result<EditSummary, CoreError> {
        self.ensure_open()?;
        let changes: Vec<ByteChange> = self.changes.values().copied().collect();
        for change in &changes {
            self.document
                .update_changed_byte_count(change.offset, change.before, change.after);
        }
        let summary = EditSummary::from_changes(self.label.clone(), &changes);
        if !changes.is_empty() {
            self.document.undo.push(EditBatch {
                label: self.label.clone(),
                changes,
            });
            self.document.redo.clear();
        }
        self.finished = true;
        Ok(summary)
    }

    /// Explicitly discard all writes.
    pub fn abort(mut self) {
        self.rollback_changes();
        self.finished = true;
    }

    fn rollback_changes(&mut self) {
        for change in self.changes.values() {
            self.document.data[change.offset] = change.before;
        }
        self.changes.clear();
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.rollback_changes();
            self.finished = true;
        }
    }
}

fn encode_integer(width: usize, endianness: Endianness, value: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(width);
    for index in 0..width {
        let shift = match endianness {
            Endianness::Little => index * 8,
            Endianness::Big => (width - 1 - index) * 8,
        };
        bytes.push((value >> shift) as u8);
    }
    bytes
}

/// Summary of a byte-by-byte comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteComparison {
    pub left_len: usize,
    pub right_len: usize,
    pub changed_bytes: usize,
    pub ranges: Vec<ByteRange>,
}

impl ByteComparison {
    pub fn identical(&self) -> bool {
        self.left_len == self.right_len && self.changed_bytes == 0
    }
}

pub fn compare_bytes(left: &[u8], right: &[u8]) -> ByteComparison {
    let mut ranges = Vec::new();
    let mut range_start = None;
    let mut changed_bytes = 0usize;
    for offset in 0..left.len().max(right.len()) {
        let differs = left.get(offset) != right.get(offset);
        if differs {
            changed_bytes += 1;
            if range_start.is_none() {
                range_start = Some(offset);
            }
        } else if let Some(start) = range_start.take() {
            ranges.push(ByteRange { start, end: offset });
        }
    }
    if let Some(start) = range_start {
        ranges.push(ByteRange {
            start,
            end: left.len().max(right.len()),
        });
    }
    ByteComparison {
        left_len: left.len(),
        right_len: right.len(),
        changed_bytes,
        ranges,
    }
}

/// Return a SHA-256 digest in lowercase hexadecimal without external dependencies.
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut message = Vec::with_capacity((data.len() + 9).div_ceil(64) * 64);
    message.extend_from_slice(data);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&(data.len() as u64 * 8).to_be_bytes());

    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in message.as_chunks::<64>().0 {
        let mut words = [0u32; 64];
        for (word, block) in words.iter_mut().zip(chunk.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*block);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }
    let mut output = String::with_capacity(64);
    for word in state {
        output.push_str(&format!("{word:08x}"));
    }
    output
}

/// Deterministically ordered string context attached to every diagnostic event.
#[derive(Debug, Clone, Default)]
pub struct LogContext {
    entries: BTreeMap<String, String>,
}

impl LogContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.entries.insert(key.into(), value.into());
    }

    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.insert(key, value);
        self
    }

    /// Render this context as a deterministic JSON object.
    pub fn to_json(&self) -> String {
        let mut output = String::from("{");
        for (index, (key, value)) in self.entries.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            output.push('"');
            output.push_str(&json_escape(key));
            output.push_str("\":\"");
            output.push_str(&json_escape(value));
            output.push('"');
        }
        output.push('}');
        output
    }
}

/// Diagnostic severity.
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

/// A JSON Lines logger designed for an agent or developer to read without debugger state.
pub struct DiagnosticLogger {
    component: String,
    operation: String,
    operation_id: String,
    base_context: LogContext,
    log_file: Option<File>,
    log_path: Option<PathBuf>,
}

impl DiagnosticLogger {
    pub fn new(
        component: impl Into<String>,
        operation: impl Into<String>,
        log_path: Option<&Path>,
    ) -> Result<Self, CoreError> {
        let log_file = match log_path {
            Some(path) => Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(|error| io_error("open diagnostic log", path, error))?,
            ),
            None => None,
        };
        let timestamp = unix_timestamp_ms();
        let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Ok(Self {
            component: component.into(),
            operation: operation.into(),
            operation_id: format!("op-{timestamp}-{}-{sequence}", std::process::id()),
            base_context: LogContext::new(),
            log_file,
            log_path: log_path.map(Path::to_path_buf),
        })
    }

    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn add_context(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.base_context.insert(key, value);
    }

    pub fn started(&mut self, message: &str, context: &LogContext) -> Result<Instant, CoreError> {
        let started = Instant::now();
        self.emit(
            LogLevel::Info,
            "started",
            "in_progress",
            message,
            context,
            None,
        )?;
        Ok(started)
    }

    pub fn complete(
        &mut self,
        message: &str,
        context: &LogContext,
        duration: Duration,
    ) -> Result<(), CoreError> {
        self.emit(
            LogLevel::Info,
            "complete",
            "success",
            message,
            context,
            Some(duration),
        )
    }

    pub fn aborted(
        &mut self,
        message: &str,
        context: &LogContext,
        duration: Duration,
    ) -> Result<(), CoreError> {
        self.emit(
            LogLevel::Error,
            "aborted",
            "failure",
            message,
            context,
            Some(duration),
        )
    }

    pub fn warning(&mut self, message: &str, context: &LogContext) -> Result<(), CoreError> {
        self.emit(
            LogLevel::Warn,
            "event",
            "in_progress",
            message,
            context,
            None,
        )
    }

    fn emit(
        &mut self,
        level: LogLevel,
        phase: &str,
        outcome: &str,
        message: &str,
        context: &LogContext,
        duration: Option<Duration>,
    ) -> Result<(), CoreError> {
        let mut merged = self.base_context.entries.clone();
        for (key, value) in &context.entries {
            merged.insert(key.clone(), value.clone());
        }
        let mut line = String::new();
        line.push('{');
        push_json_field(&mut line, "schema", LOG_SCHEMA, true);
        push_json_number(&mut line, "timestamp_ms", unix_timestamp_ms());
        push_json_field(&mut line, "level", level.as_str(), false);
        push_json_field(&mut line, "component", &self.component, false);
        push_json_field(&mut line, "operation_id", &self.operation_id, false);
        push_json_field(&mut line, "operation", &self.operation, false);
        push_json_field(&mut line, "phase", phase, false);
        push_json_field(&mut line, "outcome", outcome, false);
        push_json_field(&mut line, "message", message, false);
        line.push_str(",\"context\":");
        let context_json = LogContext { entries: merged }.to_json();
        line.push_str(&context_json);
        if let Some(duration) = duration {
            push_json_number(&mut line, "duration_ms", duration.as_millis());
        }
        line.push('}');
        eprintln!("{line}");
        if let Some(file) = self.log_file.as_mut() {
            if let Err(error) = writeln!(file, "{line}") {
                return Err(CoreError::Io {
                    operation: "write diagnostic log",
                    path: self
                        .log_path
                        .clone()
                        .unwrap_or_else(|| PathBuf::from("<diagnostic log>")),
                    message: error.to_string(),
                });
            }
            if let Err(error) = file.flush() {
                return Err(CoreError::Io {
                    operation: "flush diagnostic log",
                    path: self
                        .log_path
                        .clone()
                        .unwrap_or_else(|| PathBuf::from("<diagnostic log>")),
                    message: error.to_string(),
                });
            }
        }
        Ok(())
    }
}

fn push_json_field(output: &mut String, key: &str, value: &str, first: bool) {
    if !first {
        output.push(',');
    }
    output.push('"');
    output.push_str(&json_escape(key));
    output.push_str("\":\"");
    output.push_str(&json_escape(value));
    output.push('"');
}

fn push_json_number<T: fmt::Display>(output: &mut String, key: &str, value: T) {
    output.push(',');
    output.push('"');
    output.push_str(key);
    output.push_str("\":");
    output.push_str(&value.to_string());
}

/// Escape a string for a JSON string value.
pub fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn unix_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn unique_test_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "tuner-nook-{label}-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn typed_reads_respect_endianness_and_signed_values() {
        let document = BinDocument::from_bytes(vec![0x34, 0x12, 0xff, 0xff, 0x01, 0x02]);
        assert_eq!(
            document.read_uint(0, 2, Endianness::Little).unwrap(),
            0x1234
        );
        assert_eq!(document.read_uint(0, 2, Endianness::Big).unwrap(), 0x3412);
        assert_eq!(document.read_int(2, 2, Endianness::Little).unwrap(), -1);
        assert_eq!(document.read_uint(4, 2, Endianness::Big).unwrap(), 0x0102);
    }

    #[test]
    fn transaction_typed_reads_see_uncommitted_writes() {
        let mut document = BinDocument::from_bytes(vec![0, 0, 0, 0]);
        let mut transaction = document.transaction("typed read");
        transaction
            .write_uint(1, 2, Endianness::Little, 0x1234)
            .unwrap();
        assert_eq!(
            transaction.read_uint(1, 2, Endianness::Little).unwrap(),
            0x1234
        );
        assert_eq!(
            transaction.read_int(1, 2, Endianness::Little).unwrap(),
            0x1234
        );
    }

    #[test]
    fn out_of_bounds_write_does_not_mutate_transaction() {
        let mut document = BinDocument::from_bytes(vec![1, 2, 3]);
        {
            let mut transaction = document.transaction("failed write");
            assert!(transaction.write_u8(1, 9).is_ok());
            assert!(transaction.write_bytes(2, &[4, 5]).is_err());
        }
        assert_eq!(document.bytes(), &[1, 2, 3]);
        assert_eq!(document.undo_depth(), 0);
    }

    #[test]
    fn committed_transaction_is_one_undoable_operation() {
        let mut document = BinDocument::from_bytes(vec![0, 0, 0]);
        let summary = {
            let mut transaction = document.transaction("two bytes");
            transaction.write_u8(0, 7).unwrap();
            transaction.write_u8(2, 9).unwrap();
            transaction.commit().unwrap()
        };
        assert_eq!(summary.changed_bytes, 2);
        assert_eq!(document.bytes(), &[7, 0, 9]);
        assert_eq!(document.undo_depth(), 1);
        document.undo().unwrap();
        assert_eq!(document.bytes(), &[0, 0, 0]);
        document.redo().unwrap();
        assert_eq!(document.bytes(), &[7, 0, 9]);
    }

    #[test]
    fn dirty_byte_count_tracks_history_and_return_to_original() {
        let mut document = BinDocument::from_bytes(vec![10, 20, 30]);
        assert_eq!(document.changed_bytes, 0);
        assert_eq!(document.changed_byte_count(), 0);
        assert!(!document.is_dirty());

        {
            let mut transaction = document.transaction("dropped edit");
            transaction.write_u8(2, 31).unwrap();
        }
        assert_eq!(document.bytes(), &[10, 20, 30]);
        assert_eq!(document.changed_bytes, 0);

        let summary = {
            let mut transaction = document.transaction("net zero");
            transaction.write_u8(2, 31).unwrap();
            transaction.write_u8(2, 30).unwrap();
            transaction.commit().unwrap()
        };
        assert_eq!(summary.changed_bytes, 0);
        assert_eq!(document.undo_depth(), 0);
        assert_eq!(document.changed_bytes, 0);

        {
            let mut transaction = document.transaction("two bytes");
            transaction.write_u8(0, 11).unwrap();
            transaction.write_u8(0, 12).unwrap();
            transaction.write_u8(1, 21).unwrap();
            transaction.commit().unwrap();
        }
        assert_eq!(document.changed_bytes, 2);
        assert_eq!(document.changed_byte_count(), 2);
        assert!(document.is_dirty());

        document.undo().unwrap();
        assert_eq!(document.changed_bytes, 0);
        assert!(!document.is_dirty());
        document.redo().unwrap();
        assert_eq!(document.changed_bytes, 2);

        {
            let mut transaction = document.transaction("restore first byte");
            transaction.write_u8(0, 10).unwrap();
            transaction.commit().unwrap();
        }
        assert_eq!(document.changed_bytes, 1);
        document.undo().unwrap();
        assert_eq!(document.changed_bytes, 2);
        document.redo().unwrap();
        assert_eq!(document.changed_bytes, 1);

        {
            let mut transaction = document.transaction("restore second byte");
            transaction.write_u8(1, 20).unwrap();
            transaction.commit().unwrap();
        }
        assert_eq!(document.bytes(), &[10, 20, 30]);
        assert_eq!(document.changed_bytes, 0);
        assert_eq!(document.changed_byte_count(), 0);
        assert!(!document.is_dirty());

        document.undo().unwrap();
        assert_eq!(document.changed_bytes, 1);
        document.redo().unwrap();
        assert_eq!(document.changed_bytes, 0);
        assert!(!document.is_dirty());
    }

    #[test]
    fn edit_history_reports_undo_and_redo_actions() {
        let mut document = BinDocument::from_bytes(vec![1, 2, 3]);
        let mut transaction = document.transaction("adjust map");
        transaction.write_u8(0, 7).unwrap();
        transaction.commit().unwrap();

        assert_eq!(document.undo_history()[0].label, "adjust map");
        assert_eq!(document.undo_history()[0].changed_bytes, 1);

        document.undo().unwrap();
        assert_eq!(document.redo_history()[0].label, "adjust map");
    }

    #[test]
    fn reverting_inside_transaction_creates_no_history_change() {
        let mut document = BinDocument::from_bytes(vec![4]);
        let summary = {
            let mut transaction = document.transaction("temporary");
            transaction.write_u8(0, 8).unwrap();
            transaction.write_u8(0, 4).unwrap();
            transaction.commit().unwrap()
        };
        assert_eq!(summary.changed_bytes, 0);
        assert_eq!(document.undo_depth(), 0);
        assert_eq!(document.bytes(), &[4]);
    }

    #[test]
    fn integer_writes_validate_before_mutating() {
        let mut document = BinDocument::from_bytes(vec![0, 0]);
        {
            let mut transaction = document.transaction("range");
            assert!(transaction
                .write_uint(0, 1, Endianness::Little, 256)
                .is_err());
            assert!(transaction
                .write_int(0, 1, Endianness::Little, 128)
                .is_err());
        }
        assert_eq!(document.bytes(), &[0, 0]);
    }

    #[test]
    fn comparison_reports_contiguous_ranges_and_size_changes() {
        let comparison = compare_bytes(&[1, 2, 3, 4], &[1, 9, 3, 8, 5]);
        assert_eq!(comparison.changed_bytes, 3);
        assert_eq!(
            comparison.ranges,
            vec![
                ByteRange { start: 1, end: 2 },
                ByteRange { start: 3, end: 5 }
            ]
        );
        assert!(!comparison.identical());
    }

    #[test]
    fn sha256_matches_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn save_as_creates_output_without_overwriting() {
        let path = unique_test_path("save");
        let document = BinDocument::from_bytes(vec![10, 20, 30]);
        document.save_as(&path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), vec![10, 20, 30]);
        assert!(document.save_as(&path).is_err());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn diagnostic_log_contains_correlation_and_context() {
        let path = unique_test_path("log");
        let mut logger = DiagnosticLogger::new("test", "test.operation", Some(&path)).unwrap();
        logger.add_context("fixture", "unit-test");
        let started = logger
            .started("operation started", &LogContext::new().with("step", "load"))
            .unwrap();
        logger
            .complete(
                "operation completed",
                &LogContext::new().with("bytes", "3"),
                started.elapsed(),
            )
            .unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("\"schema\":\"tuner-log/v1\""));
        assert!(contents.contains("\"operation\":\"test.operation\""));
        assert!(contents.contains("\"fixture\":\"unit-test\""));
        assert!(contents.contains("\"duration_ms\":"));
        let _ = fs::remove_file(path);
    }
}
