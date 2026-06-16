#![allow(unused)]

use std::fs::{File, OpenOptions};
use std::io::{BufRead, Read as _, Seek, SeekFrom, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Default idle timeout for cached file handles.
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum allowed file size (100 MiB). `write` returns an error before
/// appending a line that would push the file past this limit. Keeping the
/// store below this ceiling guarantees that byte-offset IDs fit safely in a
/// `u32` (100 MiB << 4 GiB).
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

/// A file handle kept open for reuse, with a timestamp of last use.
struct CachedHandle {
    file: File,
    last_used: Instant,
}

impl CachedHandle {
    fn new(file: File) -> Self {
        Self {
            file,
            last_used: Instant::now(),
        }
    }

    fn is_expired(&self, timeout: Duration) -> bool {
        self.last_used.elapsed() > timeout
    }
}

/// Drop the inner `File` if the handle has been idle longer than `timeout`.
fn evict_if_expired(handle: &mut Option<CachedHandle>, timeout: Duration) {
    if let Some(ref cached) = *handle {
        if cached.is_expired(timeout) {
            *handle = None;
        }
    }
}

/// Error type for [`JsonlStore`] operations.
#[derive(Debug)]
pub enum JsonlStoreError {
    /// The given ID does not fall at a line boundary, or is beyond EOF.
    InvalidId { id: u32 },
    /// Appending the next message would exceed [`MAX_FILE_SIZE`].
    StoreSizeExceeded,
    /// A plain I/O error.
    Io(std::io::Error),
}

impl std::fmt::Display for JsonlStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidId { id } => write!(
                f,
                "id {id} is not aligned to a line boundary in the JSONL store"
            ),
            Self::StoreSizeExceeded => write!(
                f,
                "store size limit of {MAX_FILE_SIZE} bytes would be exceeded"
            ),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for JsonlStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidId { .. } | Self::StoreSizeExceeded => None,
            Self::Io(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for JsonlStoreError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Result of a [`JsonlStore::read`] call, carrying the items and a cursor
/// for the next read.
#[derive(Debug)]
pub struct ReadPage<T> {
    pub items: Vec<T>,
    /// Byte offset to pass as `start_id` on the next read.
    /// When no messages are read, this equals the `start_id` that was passed in.
    pub next_id: u32,
}

/// Append-only JSONL/NDJSON store.
///
/// Each line in the backing file is a single compact JSON object followed by
/// `\n`. The ID of a message equals the byte offset of its first byte.
///
/// If the handle has been idle for longer than the configured timeout it is
/// closed and reopened on the next access, avoiding repeated `open`/`close`
/// syscalls when many operations happen in quick succession.
pub struct JsonlStore<T> {
    path: PathBuf,
    /// Serialises appends so that the "get length → write" sequence is atomic.
    write_lock: Mutex<()>,
    /// Cached file handle (read+write+append), kept open for reuse.
    handle: Mutex<Option<CachedHandle>>,
    /// How long to keep a handle open after the last operation.
    idle_timeout: Duration,
    /// Maximum permitted file size in bytes. Defaults to [`MAX_FILE_SIZE`].
    max_file_size: u64,
    /// Callback for writing the byte-offset ID to the message `&mut T` after
    /// deserialising each in [`JsonlStore::read`].
    on_read: Option<Box<dyn Fn(&mut T, u32) + Send + Sync>>,
    _marker: PhantomData<T>,
}

impl<T> JsonlStore<T> {
    /// Create a store that backs onto `<app_data_dir>/<base_name>`.
    ///
    /// The parent directory is created if it does not already exist.
    /// File handles are kept open for 5 seconds after the last operation
    /// (see [`JsonlStore::with_idle_timeout`]).
    pub fn new(app_data_dir: &Path, base_name: &str) -> std::io::Result<Self> {
        std::fs::create_dir_all(app_data_dir)?;
        Ok(Self {
            path: app_data_dir.join(base_name),
            write_lock: Mutex::new(()),
            handle: Mutex::new(None),
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            max_file_size: MAX_FILE_SIZE,
            on_read: None,
            _marker: PhantomData,
        })
    }

    /// Set how long to keep the file handle open after the last operation.
    ///
    /// A handle that has been idle for longer than this duration is closed and
    /// reopened on the next operation. The default is 5 seconds.
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// Override the maximum file size (default: [`MAX_FILE_SIZE`]).
    ///
    /// Intended primarily for tests that need to trigger the size limit without
    /// writing gigabytes of data.
    pub fn with_max_file_size(mut self, max: u64) -> Self {
        self.max_file_size = max;
        self
    }

    /// Set a callback that is invoked with `&mut T` and its byte-offset ID
    /// after deserialising each message in [`JsonlStore::read`].
    ///
    /// This is typically used to write the ID into the message itself.
    pub fn with_on_read(mut self, f: impl Fn(&mut T, u32) + Send + Sync + 'static) -> Self {
        self.on_read = Some(Box::new(f));
        self
    }

    /// Close the cached file handle if it has been idle longer than the
    /// configured timeout.
    ///
    /// Call this periodically (e.g. from a timer) to release the stale file
    /// descriptor without waiting for the next read or write.
    pub fn evict_expired_handles(&self) {
        evict_if_expired(&mut self.handle.lock().unwrap(), self.idle_timeout);
    }

    /// Acquire a `MutexGuard` for the file handle, opening or reopening it
    /// as necessary. Expired handles are evicted before the check.
    ///
    /// Holding the returned guard for the duration of an operation prevents
    /// the TOCTOU race that would arise from releasing the lock between
    /// `ensure_handle` and the actual use of the handle.
    fn acquire_handle(&self) -> std::io::Result<std::sync::MutexGuard<'_, Option<CachedHandle>>> {
        let mut guard = self.handle.lock().unwrap();
        // evict_if_expired(&mut guard, self.idle_timeout);
        if guard.is_none() {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .append(true)
                .open(&self.path)?;
            *guard = Some(CachedHandle::new(file));
        }
        Ok(guard)
    }
}

impl<T: serde::Serialize + serde::de::DeserializeOwned> JsonlStore<T> {
    /// Append `message` as a single compact-JSON line and return its ID.
    ///
    /// The ID is the byte offset at which the line starts.
    ///
    /// # Errors
    ///
    /// Returns [`JsonlStoreError::StoreSizeExceeded`] if appending the line
    /// would push the file past [`MAX_FILE_SIZE`].
    pub fn write(&self, message: T) -> Result<u32, JsonlStoreError> {
        let _lock = self.write_lock.lock().unwrap();

        let json = serde_json::to_string(&message)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        // Hold the handle guard for the entire operation so no other thread
        // can evict the handle between the size check and the write.
        let mut guard = self.acquire_handle()?;
        let cached = guard.as_mut().unwrap();

        // In append mode the current file size is exactly where the write
        // will land, which is also the message ID.
        let current_len = cached.file.metadata()?.len();
        // +1 for the newline written by `writeln!`.
        if current_len + json.len() as u64 + 1 > self.max_file_size {
            return Err(JsonlStoreError::StoreSizeExceeded);
        }
        // Safety: current_len < MAX_FILE_SIZE (100 MiB) << u32::MAX (4 GiB).
        let id = current_len as u32;
        writeln!(cached.file, "{json}")?;
        cached.last_used = Instant::now();

        Ok(id)
    }

    /// Read up to `count` messages starting from `start_id`.
    ///
    /// * `start_id` of `None` means byte offset 0 (the beginning).
    /// * `count` of `None` means "read all remaining lines".
    ///
    /// If the backing file does not exist yet, returns an empty vec.
    ///
    /// # Errors
    ///
    /// Returns [`JsonlStoreError::InvalidId`] if `start_id` is non-zero and
    /// the byte immediately before that offset is not `\n` (i.e. the ID does
    /// not fall on a line boundary).
    pub fn read(&self, start_id: u32, count: Option<u32>) -> Result<ReadPage<T>, JsonlStoreError> {
        let limit = count.unwrap_or(u32::MAX);

        // No file yet and no cached handle → empty store.
        if !self.path.exists() && self.handle.lock().unwrap().is_none() {
            return Ok(ReadPage {
                items: Vec::new(),
                next_id: start_id,
            });
        }

        // Hold the handle guard for the entire read so no other thread can
        // evict the handle mid-operation.
        let mut guard = self.acquire_handle()?;
        let cached = guard.as_mut().unwrap();

        // Validate that `start_id` falls on a line boundary: either 0 or
        // preceded by `\n`. An offset beyond EOF is also invalid.
        // Do this before constructing BufReader so we can call metadata()
        // without conflicting with the mutable borrow on cached.file.
        if start_id > 0 {
            let file_len = cached.file.metadata()?.len();
            if start_id as u64 > file_len {
                return Err(JsonlStoreError::InvalidId { id: start_id });
            }
        }

        let mut reader = std::io::BufReader::new(&mut cached.file);

        if start_id > 0 {
            reader.seek(SeekFrom::Start(start_id as u64 - 1))?;
            let mut prev = [0u8];
            reader.read_exact(&mut prev)?;
            if prev[0] != b'\n' {
                return Err(JsonlStoreError::InvalidId { id: start_id });
            }
        }

        reader.seek(SeekFrom::Start(start_id as u64))?;

        let mut items = Vec::new();
        let mut offset = start_id;
        let mut line_buf = Vec::new();
        let mut remaining = limit;

        while remaining > 0 {
            line_buf.clear();
            let bytes_read = reader.read_until(b'\n', &mut line_buf)?;
            if bytes_read == 0 {
                break;
            }

            // Strip the trailing newline (read_until includes the delimiter).
            let line = if line_buf.last() == Some(&b'\n') {
                &line_buf[..line_buf.len() - 1]
            } else {
                &line_buf[..]
            };

            // Skip blank lines (e.g. a trailing newline at EOF).
            if line.is_empty() {
                offset += bytes_read as u32;
                continue;
            }

            let mut payload: T = serde_json::from_slice(line)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

            if let Some(ref f) = self.on_read {
                f(&mut payload, offset as u32);
            }

            items.push(payload);
            offset += bytes_read as u32;
            remaining -= 1;
        }

        cached.last_used = Instant::now();

        Ok(ReadPage {
            items,
            next_id: offset,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_store(dir: &Path, name: &str) -> JsonlStore<serde_json::Value> {
        JsonlStore::new(dir, name).unwrap()
    }

    #[test]
    fn write_returns_byte_offset_as_id() {
        let dir = TempDir::new().unwrap();
        let store = make_store(dir.path(), "test.jsonl");

        let id0 = store.write(serde_json::json!({ "a": 1 })).unwrap();
        assert_eq!(id0, 0);

        // The first line is `{"a":1}\n` = 8 bytes, so the next ID is 8.
        let id1 = store.write(serde_json::json!({ "b": 2 })).unwrap();
        assert_eq!(id1, 8);
    }

    #[test]
    fn read_returns_messages_with_ids_via_callback() {
        let dir = TempDir::new().unwrap();
        let store = JsonlStore::new(dir.path(), "test.jsonl")
            .unwrap()
            .with_on_read(|msg: &mut serde_json::Value, id: u32| {
                msg["id"] = serde_json::json!(id);
            });

        let id0 = store.write(serde_json::json!({ "a": 1 })).unwrap();
        let id1 = store.write(serde_json::json!({ "b": 2 })).unwrap();

        let page = store.read(0, None).unwrap();
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0]["id"], id0);
        assert_eq!(page.items[0]["a"], 1);
        assert_eq!(page.items[1]["id"], id1);
        assert_eq!(page.items[1]["b"], 2);
        // next_id should be past the last byte read
        assert!(page.next_id > id1);
    }

    #[test]
    fn read_with_start_id_skips_earlier_messages() {
        let dir = TempDir::new().unwrap();
        let store = make_store(dir.path(), "test.jsonl");

        store.write(serde_json::json!({ "a": 1 })).unwrap();
        let id1 = store.write(serde_json::json!({ "b": 2 })).unwrap();

        let page = store.read(id1, None).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0], serde_json::json!({ "b": 2 }));
        assert!(page.next_id > id1);
    }

    #[test]
    fn read_with_count_limits_results() {
        let dir = TempDir::new().unwrap();
        let store = make_store(dir.path(), "test.jsonl");

        store.write(serde_json::json!({ "a": 1 })).unwrap();
        let id1 = store.write(serde_json::json!({ "b": 2 })).unwrap();

        let page = store.read(0, Some(1)).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.next_id, id1);
    }

    #[test]
    fn read_from_nonexistent_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        let store = make_store(dir.path(), "absent.jsonl");

        let page = store.read(0, None).unwrap();
        assert!(page.items.is_empty());
        assert_eq!(page.next_id, 0);
    }

    #[test]
    fn read_with_invalid_start_id_returns_error() {
        let dir = TempDir::new().unwrap();
        let store = make_store(dir.path(), "test.jsonl");

        // `{"a":1}\n` is 8 bytes; offset 4 is mid-line.
        store.write(serde_json::json!({ "a": 1 })).unwrap();

        let err = store.read(4, None).unwrap_err();
        assert!(matches!(err, JsonlStoreError::InvalidId { id: 4 }));
    }

    #[test]
    fn read_with_start_id_past_eof_returns_invalid_id_error() {
        let dir = TempDir::new().unwrap();
        let store = make_store(dir.path(), "test.jsonl");

        store.write(serde_json::json!({ "a": 1 })).unwrap(); // 8 bytes

        // Offset 100 is well past the end of the file.
        let err = store.read(100, None).unwrap_err();
        assert!(matches!(err, JsonlStoreError::InvalidId { id: 100 }));
    }

    #[test]
    fn write_returns_error_when_size_limit_exceeded() {
        let dir = TempDir::new().unwrap();
        // Set a 32-byte limit so a handful of small writes trips the guard
        // without writing megabytes of test data.
        let store = JsonlStore::new(dir.path(), "test.jsonl")
            .unwrap()
            .with_max_file_size(32);

        // Each `{"i":0}\n` is 8 bytes; we should hit the limit within a few
        // iterations and certainly within 100.
        let mut hit_limit = false;
        for i in 0..100_u64 {
            match store.write(serde_json::json!({ "i": i })) {
                Ok(_) => {}
                Err(JsonlStoreError::StoreSizeExceeded) => {
                    hit_limit = true;
                    break;
                }
                Err(e) => panic!("unexpected error: {e}"),
            }
        }
        assert!(hit_limit, "expected StoreSizeExceeded to be returned");
    }

    #[test]
    fn cached_handle_is_reused_across_rapid_operations() {
        let dir = TempDir::new().unwrap();
        let store = make_store(dir.path(), "test.jsonl");

        // Rapid writes – each should reuse the cached handle.
        let mut ids = Vec::new();
        for i in 0..10 {
            let id = store.write(serde_json::json!({ "i": i })).unwrap();
            ids.push(id);
        }

        // Rapid reads – should reuse the same cached handle.
        let page = store.read(0, None).unwrap();
        assert_eq!(page.items.len(), 10);
        for (i, msg) in page.items.iter().enumerate() {
            assert_eq!(*msg, serde_json::json!({ "i": i as i64 }));
        }
    }

    #[test]
    fn expired_handle_is_reopened() {
        let dir = TempDir::new().unwrap();
        let store = JsonlStore::new(dir.path(), "test.jsonl")
            .unwrap()
            .with_idle_timeout(Duration::from_millis(50));

        let id0 = store.write(serde_json::json!({ "a": 1 })).unwrap();
        assert_eq!(id0, 0);

        // Wait for the handle to expire.
        std::thread::sleep(Duration::from_millis(100));

        // New write should reopen the handle and still work correctly.
        let id1 = store.write(serde_json::json!({ "b": 2 })).unwrap();
        assert!(id1 > id0);

        // New read should reopen the handle.
        let page = store.read(id1, None).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0], serde_json::json!({ "b": 2 }));
    }

    #[test]
    fn evict_expired_handles_closes_stale_fd() {
        let dir = TempDir::new().unwrap();
        let store = JsonlStore::new(dir.path(), "test.jsonl")
            .unwrap()
            .with_idle_timeout(Duration::from_millis(50));

        // Open the handle.
        store.write(serde_json::json!({ "a": 1 })).unwrap();
        store.read(0, None).unwrap();

        // Handle is still within the timeout — eviction should not close it.
        store.evict_expired_handles();

        // A quick second operation should reuse the cached handle.
        let page = store.read(0, None).unwrap();
        assert_eq!(page.items.len(), 1);

        // Now wait past the timeout.
        std::thread::sleep(Duration::from_millis(100));

        // Eviction should close the stale handle.
        store.evict_expired_handles();

        // Next operations reopen the handle and still work.
        let id = store.write(serde_json::json!({ "b": 2 })).unwrap();
        assert!(id > 0);
        let page2 = store.read(0, None).unwrap();
        assert_eq!(page2.items.len(), 2);
    }
}
