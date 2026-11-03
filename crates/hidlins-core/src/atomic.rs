//! Atomic file-write helpers.
//!
//! Per design §2.2.4 and FR-054 / NFR-006: every on-disk mutation in
//! `hidlins-core` (KDBX vault saves, `vaults.toml` updates) funnels
//! through this module's [`write_atomic`] function. The algorithm is:
//!
//! 1. Lock the parent directory briefly, reclaim recognized abandoned stages,
//!    and create a private `0700` sibling staging directory with a locked,
//!    durable ownership marker. Unmarked legacy objects are preserved.
//! 2. Release setup coordination and stream encrypted bytes into its `0600`
//!    payload file; independent writers can serialize in parallel.
//! 3. `fsync` the payload and rename it onto the target on the same filesystem.
//! 4. Persist source-directory cleanup and the destination parent directory.
//!
//! Staging traversal and mutations are relative to held directory descriptors,
//! with no-follow opens and inode checks. Ordinary errors clean owned artifacts
//! through RAII. After process death the next write reclaims recognized stages;
//! objects without an established marker remain unrecognized and untouched.

use std::ffi::{OsStr, OsString};
use std::fs::{File, TryLockError};
use std::io::{self, Read as _, Write as _};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::Path;

use rustix::fs::{
    mkdirat, openat, renameat, statat, unlinkat, AtFlags, Dir, FileType, Mode, OFlags,
};
use tempfile::Builder as TempFileBuilder;

use crate::locking::{lock_exclusive, try_lock_exclusive, unlock_file};
use crate::VaultError;

const TEMP_PREFIX: &str = ".hidlins-tmp-";
const OWNER: &str = "owner";
const PAYLOAD: &str = "payload";
const OWNER_MARKER: &[u8] = b"hidlins-atomic-stage/v1\n";

/// Atomically replace the file at `target` with `bytes`.
///
/// See the module-level [`self`] doc for the full algorithm.
///
/// # Errors
///
/// Returns [`VaultError::Io`] if any filesystem operation fails, including:
/// - inability to create a temp file in the parent directory
/// - write failure (disk-full, permission denied)
/// - `rename(2)` failure
/// - parent-directory `fsync` failure
pub fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), VaultError> {
    write_atomic_with(target, |file| {
        file.write_all(bytes).map_err(|source| VaultError::Io {
            source,
            path: target.to_path_buf(),
        })
    })
}

/// Streaming variant of [`write_atomic`].
///
/// Instead of buffering all bytes in a `&[u8]`, accepts a closure that
/// writes directly into the temp file. This is used by the KDBX save path
/// so the entire encrypted vault never has to live in a single heap buffer.
///
/// The closure receives a mutable reference to the temp file; it should
/// write all desired bytes and return `Ok(())` on success. On error, the
/// temp file is cleaned up automatically.
///
/// # Errors
///
/// Same as [`write_atomic`] — returns [`VaultError::Io`] on any filesystem
/// failure. The `path` field in the error reflects the *target* path.
pub fn write_atomic_with<F>(target: &Path, write: F) -> Result<(), VaultError>
where
    F: FnOnce(&mut File) -> Result<(), VaultError>,
{
    write_atomic_with_commit(target, write, || {})
}

/// Notify an internal owner immediately after rename succeeds, before any
/// fallible durability work. This keeps live credentials bound to the bytes
/// actually installed even when a later operation reports an error.
pub(crate) fn write_atomic_with_commit<F, C>(
    target: &Path,
    write: F,
    committed: C,
) -> Result<(), VaultError>
where
    F: FnOnce(&mut File) -> Result<(), VaultError>,
    C: FnOnce(),
{
    let mut stage = AtomicStage::new(target).map_err(|source| VaultError::Io {
        source,
        path: target.to_path_buf(),
    })?;
    write(&mut stage.files.payload)?;
    maybe_signal_fault_phase(target, AtomicWritePhase::TempWritten)?;
    stage
        .files
        .payload
        .sync_all()
        .map_err(|source| VaultError::Io {
            source,
            path: target.to_path_buf(),
        })?;
    maybe_signal_fault_phase(target, AtomicWritePhase::FsyncDone)?;

    // Both names are relative to held directory descriptors. A pathname
    // rebind cannot redirect this replacement or subsequent source cleanup.
    renameat(
        &stage.files.directory,
        PAYLOAD,
        &stage.parent,
        target.file_name().expect("validated filename"),
    )
    .map_err(|source| VaultError::Io {
        source: source.into(),
        path: target.to_path_buf(),
    })?;
    committed();
    maybe_signal_fault_phase(target, AtomicWritePhase::RenameDone)?;
    stage.finish().map_err(|source| VaultError::Io {
        source,
        path: target.to_path_buf(),
    })
}

/// Open `parent` and call `sync_all()` on it (fsync the directory).
///
/// This makes renames inside the directory durable across power loss.
/// On APFS / macOS this may be a no-op, but we invoke it unconditionally
/// for POSIX portability and clarity (design §4.5 in implementation-plan).
pub(crate) fn sync_parent_dir(parent: &Path) -> io::Result<()> {
    let dir = File::open(normalize_parent(parent))?;
    dir.sync_all()
}

/// Replace `target` with an already-fsynced sibling stage and fsync the
/// containing directory. Both paths must have the same parent; callers use
/// this only for internally derived transaction artifacts.
pub(crate) fn install_staged_file(stage: &Path, target: &Path) -> Result<(), VaultError> {
    let stage_parent = stage.parent().map(normalize_parent);
    let target_parent = target.parent().map(normalize_parent);
    if stage_parent.is_none() || stage_parent != target_parent {
        return Err(VaultError::PasswordRotationRecoveryFailed {
            reason: "transaction stage is not a sibling of its target",
        });
    }
    std::fs::rename(stage, target).map_err(|source| VaultError::Io {
        source,
        path: target.to_path_buf(),
    })?;
    sync_parent_dir(target_parent.expect("checked parent above")).map_err(|source| VaultError::Io {
        source,
        path: target_parent.expect("checked parent above").to_path_buf(),
    })
}

/// Remove an artifact and fsync its containing directory. A missing artifact
/// is already removed and therefore succeeds.
pub(crate) fn remove_file_durable(path: &Path) -> Result<(), VaultError> {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(VaultError::Io {
                source,
                path: path.to_path_buf(),
            })
        }
    }
    let parent =
        path.parent()
            .map(normalize_parent)
            .ok_or(VaultError::PasswordRotationRecoveryFailed {
                reason: "transaction artifact has no parent directory",
            })?;
    sync_parent_dir(parent).map_err(|source| VaultError::Io {
        source,
        path: parent.to_path_buf(),
    })
}

fn normalize_parent(parent: &Path) -> &Path {
    // A bare filename has an empty parent, which denotes the current
    // directory but is not accepted by filesystem operations like read_dir.
    if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    }
}

/// A live stage's marker stays locked until all its mutations finish.
struct StageFiles {
    name: OsString,
    directory: File,
    owner: File,
    payload: File,
}

struct AtomicStage {
    parent: File,
    files: StageFiles,
    finished: bool,
}

impl AtomicStage {
    fn new(target: &Path) -> io::Result<Self> {
        // Path::file_name normalizes trailing separators and `/.`. Those
        // spellings require a directory in the original OS pathname and
        // must never be reduced to a replaceable file basename.
        let raw = target.as_os_str().as_bytes();
        if target.file_name().is_none() || raw.ends_with(b"/") || raw.ends_with(b"/.") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "target has no filename",
            ));
        }
        let parent_path = target.parent().map(normalize_parent).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "target has no parent directory",
            )
        })?;
        let parent = File::open(parent_path)?;
        let guard = ParentGuard::new(&parent)?;
        cleanup_stale_temp_files(&parent)?;
        // Retain tempfile's exclusive random-name allocation/retry behavior;
        // actual creation is descriptor-relative and directory-only.
        let candidate = TempFileBuilder::new()
            .prefix(TEMP_PREFIX)
            .rand_bytes(16)
            .disable_cleanup(true)
            .make_in(parent_path, |path| {
                let name = path.file_name().expect("generated filename").to_owned();
                mkdirat(&parent, &name, Mode::RWXU)?;
                let directory = open_directory(&parent, &name)?;
                let mut owner = create_private_file(&directory, OWNER)?;
                try_lock_exclusive(&owner).map_err(lock_error)?;
                owner.write_all(OWNER_MARKER)?;
                owner.sync_all()?;
                directory.sync_all()?;
                // No payload can be written before its durable ownership
                // marker exists and is locked by this writer.
                let payload = create_private_file(&directory, PAYLOAD)?;
                Ok(StageFiles {
                    name,
                    directory,
                    owner,
                    payload,
                })
            });
        if candidate.is_err() {
            // A failure after marker establishment can already be reclaimed.
            // Untagged empty setup artifacts remain unrecognized and untouched.
            let _ = cleanup_stale_temp_files(&parent);
        }
        drop(guard);
        let (files, unused_path) = candidate?.into_parts();
        drop(unused_path); // disable_cleanup(true): deletion is descriptor-relative
        Ok(Self {
            parent,
            files,
            finished: false,
        })
    }

    fn finish(&mut self) -> io::Result<()> {
        // The live marker lock already excludes orphan cleanup. Finalizing
        // our own descriptor-bound stage needs no parent-wide coordination;
        // other writers can set up or finish their stages concurrently.
        let cleanup = cleanup_open_stage(
            &self.parent,
            &self.files.name,
            &self.files.directory,
            &self.files.owner,
            Some(&self.files.payload),
        );
        // Make the installed target durable even if source cleanup fails.
        let durable = self.parent.sync_all();
        cleanup?;
        durable?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for AtomicStage {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.finish();
        }
    }
}

/// Serializes orphan discovery and stage creation, never encryption.
struct ParentGuard<'a>(&'a File);

impl<'a> ParentGuard<'a> {
    fn new(file: &'a File) -> io::Result<Self> {
        lock_exclusive(file)?;
        Ok(Self(file))
    }
}

impl Drop for ParentGuard<'_> {
    fn drop(&mut self) {
        let _ = unlock_file(self.0);
    }
}

fn open_directory(parent: &File, name: &OsStr) -> io::Result<File> {
    Ok(openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?
    .into())
}

fn create_private_file(directory: &File, name: &str) -> io::Result<File> {
    Ok(openat(
        directory,
        name,
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )?
    .into())
}

fn open_regular_file(directory: &File, name: &str) -> io::Result<File> {
    let metadata = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW)?;
    if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "stage child is not a regular file",
        ));
    }
    // NONBLOCK also closes the type-check/open race for a substituted FIFO.
    Ok(openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )?
    .into())
}

fn private_directory(file: &File, uid: u32) -> io::Result<bool> {
    let metadata = file.metadata()?;
    Ok(metadata.is_dir()
        && metadata.uid() == uid
        && metadata.permissions().mode() & 0o7777 == 0o700)
}

fn private_file(file: &File, uid: u32) -> io::Result<bool> {
    let metadata = file.metadata()?;
    Ok(metadata.is_file()
        && metadata.uid() == uid
        && metadata.nlink() == 1
        && metadata.permissions().mode() & 0o7777 == 0o600)
}

fn same_file(left: &File, right: &File) -> io::Result<bool> {
    let left = left.metadata()?;
    let right = right.metadata()?;
    Ok(left.dev() == right.dev() && left.ino() == right.ino())
}

fn known_children(directory: &File) -> io::Result<bool> {
    for entry in Dir::read_from(directory)? {
        let entry = entry?;
        if !matches!(
            entry.file_name().to_bytes(),
            b"." | b".." | b"owner" | b"payload"
        ) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn cleanup_stale_temp_files(parent: &File) -> io::Result<()> {
    let uid = effective_uid();
    for entry in Dir::read_from(parent)? {
        let entry = entry?;
        let name = OsStr::from_bytes(entry.file_name().to_bytes());
        if !name.as_bytes().starts_with(TEMP_PREFIX.as_bytes()) {
            continue;
        }
        // Ordinary files (including prefixed live vaults/keyfiles), links,
        // inaccessible directories and untagged legacy objects are not ours.
        let Ok(directory) = open_directory(parent, name) else {
            continue;
        };
        if !private_directory(&directory, uid)? {
            continue;
        }
        let Ok(mut owner) = open_regular_file(&directory, OWNER) else {
            continue;
        };
        if !private_file(&owner, uid)? || owner.metadata()?.len() != OWNER_MARKER.len() as u64 {
            continue;
        }
        match try_lock_exclusive(&owner) {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => continue,
            Err(TryLockError::Error(error)) => return Err(error),
        }
        // A live writer may have finalized after our initial metadata check
        // and before this lock was acquired. Its unlinked marker is no longer
        // evidence that the named directory still needs reclaiming.
        if !private_file(&owner, uid)? {
            continue;
        }
        let Ok(named_owner) = open_regular_file(&directory, OWNER) else {
            continue;
        };
        if !same_file(&named_owner, &owner)? {
            continue;
        }
        let mut marker = [0; OWNER_MARKER.len()];
        if owner.read_exact(&mut marker).is_err() || marker != OWNER_MARKER {
            continue;
        }
        if !known_children(&directory)? {
            continue;
        }
        let payload = match open_regular_file(&directory, PAYLOAD) {
            Ok(file) if private_file(&file, uid)? => Some(file),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            _ => continue,
        };
        cleanup_open_stage(parent, name, &directory, &owner, payload.as_ref())?;
        parent.sync_all()?;
    }
    Ok(())
}

fn remove_owned_child(directory: &File, name: &str, expected: Option<&File>) -> io::Result<()> {
    let current = match open_regular_file(directory, name) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !expected
        .map(|file| same_file(&current, file))
        .transpose()?
        .unwrap_or(false)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "stage child identity changed",
        ));
    }
    unlinkat(directory, name, AtFlags::empty())?;
    Ok(())
}

fn cleanup_open_stage(
    parent: &File,
    name: &OsStr,
    directory: &File,
    owner: &File,
    payload: Option<&File>,
) -> io::Result<()> {
    if !known_children(directory)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unrecognized stage child",
        ));
    }
    remove_owned_child(directory, PAYLOAD, payload)?;
    remove_owned_child(directory, OWNER, Some(owner))?;
    // Persist source-directory mutations before unlinking the empty stage.
    directory.sync_all()?;
    match open_directory(parent, name) {
        Ok(named) if same_file(&named, directory)? => unlinkat(parent, name, AtFlags::REMOVEDIR)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "stage directory identity changed",
            ))
        }
    }
    Ok(())
}

fn lock_error(error: TryLockError) -> io::Error {
    match error {
        TryLockError::WouldBlock => io::Error::new(
            io::ErrorKind::WouldBlock,
            "new stage marker unexpectedly locked",
        ),
        TryLockError::Error(error) => error,
    }
}

#[allow(unsafe_code)] // audited argument-free POSIX identity query
fn effective_uid() -> u32 {
    // SAFETY: geteuid has no arguments, pointers, or ownership effects.
    unsafe { libc::geteuid() }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtomicWritePhase {
    TempWritten,
    FsyncDone,
    RenameDone,
}

#[cfg(debug_assertions)]
impl AtomicWritePhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::TempWritten => "temp_written",
            Self::FsyncDone => "fsync_done",
            Self::RenameDone => "rename_done",
        }
    }
}

#[cfg(debug_assertions)]
fn maybe_signal_fault_phase(target: &Path, phase: AtomicWritePhase) -> Result<(), VaultError> {
    const TARGET_ENV: &str = "HIDLINS_ATOMIC_FAULT_TARGET";
    const SIGNAL_DIR_ENV: &str = "HIDLINS_ATOMIC_FAULT_SIGNAL_DIR";
    const PAUSE_PHASE_ENV: &str = "HIDLINS_ATOMIC_FAULT_PAUSE_PHASE";

    let Ok(expected_target) = std::env::var(TARGET_ENV) else {
        return Ok(());
    };

    if target != Path::new(&expected_target) {
        let expected = crate::locking::resolve_file_path(Path::new(&expected_target)).ok();
        let actual = crate::locking::resolve_file_path(target).ok();
        if expected.is_none() || expected != actual {
            return Ok(());
        }
    }

    if let Ok(signal_dir) = std::env::var(SIGNAL_DIR_ENV) {
        let signal_path = Path::new(&signal_dir).join(phase.as_str());
        std::fs::write(&signal_path, phase.as_str()).map_err(|source| VaultError::Io {
            source,
            path: signal_path,
        })?;
    }

    if std::env::var(PAUSE_PHASE_ENV).as_deref() == Ok(phase.as_str()) {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    Ok(())
}

#[cfg(not(debug_assertions))]
fn maybe_signal_fault_phase(_target: &Path, _phase: AtomicWritePhase) -> Result<(), VaultError> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, set_permissions, Permissions};
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn write_atomic_creates_new_file_with_mode_0600() {
        let dir = tempfile::TempDir::new().expect("create tempdir");
        let target = dir.path().join("new_file.bin");

        write_atomic(&target, b"hello").expect("write_atomic succeeded");

        assert!(target.exists());
        assert_eq!(read_bytes(&target), b"hello");

        #[cfg(unix)]
        {
            let metadata = target.metadata().expect("stat the file");
            let mode = metadata.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "file mode should be 0600, got {mode:#o}");
        }
    }

    #[test]
    fn write_atomic_replaces_existing_file() {
        let dir = tempfile::TempDir::new().expect("create tempdir");
        let target = dir.path().join("replace.bin");

        // Write initial content.
        fs::write(&target, b"old data").expect("initial write");
        assert_eq!(read_bytes(&target), b"old data");

        // Atomic replace.
        write_atomic(&target, b"new data").expect("write_atomic succeeded");

        assert_eq!(read_bytes(&target), b"new data");
    }

    #[test]
    fn write_atomic_leaves_target_intact_on_pre_rename_failure() {
        let dir = tempfile::TempDir::new().expect("create tempdir");
        let target = dir.path().join("stable.bin");

        // Write initial content.
        fs::write(&target, b"original").expect("initial write");

        // Make the parent directory read-only so the temp file can be created
        // but persist() (rename) will fail.
        let old_perm = dir.path().metadata().unwrap().permissions().mode() & 0o777;
        set_permissions(dir.path(), Permissions::from_mode(0o500)).expect("chmod parent read-only");

        // The write should fail.
        let result = write_atomic(&target, b"overwritten");
        assert!(
            result.is_err(),
            "write_atomic should fail on read-only parent"
        );

        // Restore permissions so TempDir can clean up.
        set_permissions(dir.path(), Permissions::from_mode(old_perm)).expect("restore permissions");

        // The target must still be byte-identical to its original content.
        assert_eq!(read_bytes(&target), b"original");
    }

    #[test]
    fn write_atomic_cleans_up_temp_on_failure() {
        let dir = tempfile::TempDir::new().expect("create tempdir");

        // Make the parent directory read-only — prevents persist/rename.
        let old_perm = dir.path().metadata().unwrap().permissions().mode() & 0o777;
        set_permissions(dir.path(), Permissions::from_mode(0o500)).expect("chmod parent read-only");

        // Attempt a write that will fail during persist.
        let target = dir.path().join("should_fail.bin");
        let _ = write_atomic(&target, b"nope");

        // Restore permissions for cleanup.
        set_permissions(dir.path(), Permissions::from_mode(old_perm)).expect("restore permissions");

        // No orphan .tmp* files should remain in the parent dir.
        let orphans: Vec<_> = fs::read_dir(dir.path())
            .expect("read tempdir")
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with(".tmp"))
            .collect();
        assert!(
            orphans.is_empty(),
            "orphan temp files found: {:?}",
            orphans
                .iter()
                .map(std::fs::DirEntry::file_name)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn write_atomic_writes_zero_bytes_correctly() {
        let dir = tempfile::TempDir::new().expect("create tempdir");
        let target = dir.path().join("empty.bin");

        write_atomic(&target, &[]).expect("write_atomic with empty slice succeeded");

        assert!(target.exists());
        assert_eq!(fs::metadata(&target).unwrap().len(), 0);
    }

    #[test]
    fn write_atomic_writes_large_buffer_correctly() {
        let dir = tempfile::TempDir::new().expect("create tempdir");
        let target = dir.path().join("large.bin");

        // 10 MiB — typical for KDBX files with attachments.
        let data: Vec<u8> = (0_u32..10 * 1024 * 1024)
            .map(|i| u8::try_from(i & 0xff).expect("i & 0xff fits in u8"))
            .collect();

        write_atomic(&target, &data).expect("write_atomic large buffer succeeded");

        let written = fs::read(&target).expect("read target after atomic write");
        assert_eq!(written.len(), data.len());
        assert_eq!(written, data);
    }

    #[test]
    fn write_atomic_with_streaming_path_writes_bytes() {
        let dir = tempfile::TempDir::new().expect("create tempdir");
        let target = dir.path().join("streaming.bin");

        let data = b"streaming test content";

        write_atomic_with(&target, |f| {
            f.write_all(data).map_err(|source| VaultError::Io {
                source,
                path: target.clone(),
            })
        })
        .expect("write_atomic_with succeeded");

        assert_eq!(read_bytes(&target), data);
    }

    #[test]
    fn write_atomic_streaming_parity() {
        // Confirm that write_atomic_with produces the same result as
        // write_atomic for an identical byte payload.
        let dir = tempfile::TempDir::new().expect("create tempdir");
        let target_buf = dir.path().join("buffered.bin");
        let target_stream = dir.path().join("streaming.bin");

        let data: Vec<u8> = (0_u32..4096)
            .map(|i| u8::try_from(i & 0xff).expect("i & 0xff fits in u8"))
            .collect();

        write_atomic(&target_buf, &data).expect("buffered write succeeded");
        write_atomic_with(&target_stream, |f| {
            f.write_all(&data).map_err(|source| VaultError::Io {
                source,
                path: target_stream.clone(),
            })
        })
        .expect("streaming write succeeded");

        assert_eq!(
            fs::read(&target_buf).unwrap(),
            fs::read(&target_stream).unwrap()
        );
    }

    #[test]
    fn write_atomic_target_in_nonexistent_parent_dir_errors() {
        let dir = tempfile::TempDir::new().expect("create tempdir");
        // Use a path whose parent does not exist.
        let target = dir
            .path()
            .join("nonexistent")
            .join("subdir")
            .join("file.bin");

        let result = write_atomic(&target, b"data");
        assert!(
            result.is_err(),
            "write_atomic should fail on missing parent dir"
        );
    }

    #[test]
    fn directory_syntax_cannot_overwrite_a_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let target = dir.path().join("stable.bin");
        for suffix in ["/", "/.", "//", "/./"] {
            for streaming in [false, true] {
                fs::write(&target, b"original").unwrap();
                let mut raw = target.as_os_str().to_owned();
                raw.push(suffix);
                let invalid = std::path::PathBuf::from(raw);
                let result = if streaming {
                    write_atomic_with(&invalid, |file| {
                        file.write_all(b"unexpected")
                            .map_err(|source| VaultError::Io {
                                source,
                                path: invalid.clone(),
                            })
                    })
                } else {
                    write_atomic(&invalid, b"unexpected")
                };
                assert!(result.is_err(), "directory syntax accepted: {invalid:?}");
                assert_eq!(fs::read(&target).unwrap(), b"original");
            }
        }
    }

    fn read_bytes(path: &Path) -> Vec<u8> {
        fs::read(path).expect("read file")
    }
}
