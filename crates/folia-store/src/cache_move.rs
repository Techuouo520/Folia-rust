// Verified cache transfers with a durable copy/cleanup journal and cooperative root locks.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{StorageSettings, Store, StoreError};

const DIRECTORIES: [&str; 2] = ["audio", "covers"];
const DATABASES: [&str; 3] = ["room_sound.sqlite", "room_sound.sqlite-wal", "room_sound.sqlite-shm"];

#[derive(Debug)]
pub struct CacheLease {
    _file: File,
}

impl CacheLease {
    pub fn shared(root: &Path) -> Result<Self, StoreError> {
        Self::open(root, true)
    }

    fn open(root: &Path, shared: bool) -> Result<Self, StoreError> {
        fs::create_dir_all(root).map_err(|error| io_error(root, error))?;
        let path = root.join(".folia-cache.lock");
        if path.exists() {
            regular(&path)?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| io_error(&path, error))?;
        let result = if shared { file.try_lock_shared() } else { file.try_lock() };
        result.map_err(|error| io_error(root, error.into()))?;
        Ok(Self { _file: file })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheMigration {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub restore_default: bool,
    pub staging: String,
    #[serde(default)]
    pub cleanup: bool,
}

#[derive(Debug, Clone)]
pub struct CacheMoveResult {
    pub settings: StorageSettings,
    pub cleanup_pending: bool,
}

impl StorageSettings {
    /// Call only after the host has stopped cache writers and released its shared leases.
    pub fn migrate(
        store: &Store,
        source: &Path,
        directory: Option<PathBuf>,
        default: &Path,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<CacheMoveResult, StoreError> {
        let _transaction = transaction(store)?;
        let mut settings = Self::load(store)?;
        if let Some(previous) = &settings.migration {
            if previous.cleanup {
                let finished = Self::finish_migration_inner(store, &mut progress)?;
                if finished.cleanup_pending {
                    return Err(invalid(source, "previous cache cleanup is incomplete"));
                }
                settings = finished.settings;
            } else {
                validate_plan(previous)?;
                remove_stage(previous)?;
                settings.migration = None;
            }
        }
        let target = directory.as_deref().unwrap_or(default);
        if !source.is_absolute() || !target.is_absolute() {
            return Err(invalid(target, "cache paths must be absolute"));
        }
        fs::create_dir_all(source).map_err(|error| io_error(source, error))?;
        fs::create_dir_all(target).map_err(|error| io_error(target, error))?;
        let source = fs::canonicalize(source).map_err(|error| io_error(source, error))?;
        let destination = fs::canonicalize(target).map_err(|error| io_error(target, error))?;
        let configured = settings.cache_path(default);
        if fs::canonicalize(configured).map_err(|error| io_error(configured, error))? != source {
            return Err(invalid(&source, "cache preferences changed in another instance"));
        }
        if source == destination {
            settings.cache_directory = directory;
            store.save(&settings)?;
            progress(0, 0);
            return Ok(CacheMoveResult { settings, cleanup_pending: false });
        }
        if source.starts_with(&destination) || destination.starts_with(&source) {
            return Err(invalid(target, "cache roots must not contain each other"));
        }
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
        let staging = format!("{:x}-{stamp:x}", std::process::id());
        settings.migration =
            Some(CacheMigration { source, destination, restore_default: directory.is_none(), staging, cleanup: false });
        store.save(&settings)?;
        Self::finish_migration_inner(store, progress)
    }

    /// Replays an interrupted transfer before cache consumers start, or retries pending cleanup.
    pub fn finish_migration(store: &Store, mut progress: impl FnMut(u64, u64)) -> Result<CacheMoveResult, StoreError> {
        let _transaction = transaction(store)?;
        Self::finish_migration_inner(store, &mut progress)
    }

    fn finish_migration_inner(
        store: &Store,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<CacheMoveResult, StoreError> {
        let mut settings = Self::load(store)?;
        let Some(plan) = settings.migration.clone() else {
            return Ok(CacheMoveResult { settings, cleanup_pending: false });
        };
        validate_plan(&plan)?;
        let _source = CacheLease::open(&plan.source, false)?;
        let _destination = CacheLease::open(&plan.destination, false)?;
        let files = cache_files(&plan.source)?;
        let total = files.iter().map(|(_, size)| size).sum();
        let mut done = 0;
        progress(done, total);
        if !plan.cleanup {
            for (relative, _) in &files {
                let to = plan.destination.join(relative);
                checked_parent(&plan.destination, relative, false)?;
                if to.exists() && !same(&plan.source.join(relative), &to)? {
                    return Err(invalid(&to, "a different file already exists at the destination"));
                }
            }
            for (relative, size) in &files {
                transfer(&plan, relative, |bytes| progress(done + bytes, total))?;
                done += size;
                progress(done, total);
            }
            if cache_files(&plan.source)? != files {
                return Err(invalid(&plan.source, "cache changed while copying"));
            }
            settings.cache_directory = (!plan.restore_default).then(|| plan.destination.clone());
            settings.migration = Some(CacheMigration { cleanup: true, ..plan.clone() });
            store.save(&settings)?;
        }
        if let Err(error) = cleanup(&plan, &files) {
            tracing::warn!(error = &error as &dyn std::error::Error, "cache moved; old files need cleanup");
            return Ok(CacheMoveResult { settings, cleanup_pending: true });
        }
        settings.migration = None;
        if let Err(error) = store.save(&settings) {
            tracing::warn!(error = &error as &dyn std::error::Error, "cache moved; cleanup journal remains");
            settings.migration = Some(CacheMigration { cleanup: true, ..plan });
            return Ok(CacheMoveResult { settings, cleanup_pending: true });
        }
        tracing::info!(files = files.len(), bytes = total, "cache directory migrated");
        Ok(CacheMoveResult { settings, cleanup_pending: false })
    }
}

fn validate_plan(plan: &CacheMigration) -> Result<(), StoreError> {
    if plan.staging.is_empty()
        || plan.staging.len() > 80
        || !plan.staging.bytes().all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    {
        return Err(invalid(&plan.destination, "invalid migration staging id"));
    }
    if !plan.source.is_absolute() || !plan.destination.is_absolute() {
        return Err(invalid(&plan.destination, "cache paths must be absolute"));
    }
    let source = fs::canonicalize(&plan.source).map_err(|error| io_error(&plan.source, error))?;
    let destination = fs::canonicalize(&plan.destination).map_err(|error| io_error(&plan.destination, error))?;
    if source != plan.source
        || destination != plan.destination
        || source.starts_with(&destination)
        || destination.starts_with(&source)
    {
        return Err(invalid(&plan.destination, "cache root changed or overlaps the source"));
    }
    Ok(())
}

fn cache_files(root: &Path) -> Result<Vec<(PathBuf, u64)>, StoreError> {
    let mut files = Vec::new();
    for name in DIRECTORIES.into_iter().chain(DATABASES) {
        let path = root.join(name);
        if path.try_exists().map_err(|error| io_error(&path, error))? {
            collect(root, &path, &mut files)?;
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

fn collect(root: &Path, path: &Path, files: &mut Vec<(PathBuf, u64)>) -> Result<(), StoreError> {
    let metadata = regular(path)?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|error| io_error(path, error))? {
            collect(root, &entry.map_err(|error| io_error(path, error))?.path(), files)?;
        }
    } else if metadata.is_file() {
        let relative = path.strip_prefix(root).map_err(|_| invalid(path, "file escaped cache root"))?;
        files.push((relative.into(), metadata.len()));
    } else {
        return Err(invalid(path, "cache entry is not a regular file"));
    }
    Ok(())
}

fn regular(path: &Path) -> Result<fs::Metadata, StoreError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let linked = metadata.file_type().is_symlink();
    if linked { Err(invalid(path, "linked cache entries cannot be migrated")) } else { Ok(metadata) }
}

fn same(first: &Path, second: &Path) -> Result<bool, StoreError> {
    let left = regular(first)?;
    let right = regular(second)?;
    if !left.is_file() || !right.is_file() || left.len() != right.len() {
        return Ok(false);
    }
    let mut a = File::open(first).map_err(|error| io_error(first, error))?;
    let mut b = File::open(second).map_err(|error| io_error(second, error))?;
    let mut left = [0; 64 * 1024];
    let mut right = [0; 64 * 1024];
    loop {
        let length = a.read(&mut left).map_err(|error| io_error(first, error))?;
        if length == 0 {
            return b.read(&mut right[..1]).map(|length| length == 0).map_err(|error| io_error(second, error));
        }
        b.read_exact(&mut right[..length]).map_err(|error| io_error(second, error))?;
        if left[..length] != right[..length] {
            return Ok(false);
        }
    }
}

fn transfer(plan: &CacheMigration, relative: &Path, mut progress: impl FnMut(u64)) -> Result<(), StoreError> {
    let source = plan.source.join(relative);
    let destination = plan.destination.join(relative);
    if destination.exists() {
        return Ok(());
    }
    let stage = stage_root(plan);
    checked_parent(
        &plan.destination,
        &stage.strip_prefix(&plan.destination).map_err(|_| invalid(&stage, "invalid staging path"))?.join(relative),
        true,
    )?;
    let temporary = stage.join(relative);
    if temporary.exists() {
        regular(&temporary)?;
    }
    let mut input = File::open(&source).map_err(|error| io_error(&source, error))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temporary)
        .map_err(|error| io_error(&temporary, error))?;
    let mut buffer = [0; 64 * 1024];
    let mut copied = 0;
    loop {
        let size = input.read(&mut buffer).map_err(|error| io_error(&source, error))?;
        if size == 0 {
            break;
        }
        output.write_all(&buffer[..size]).map_err(|error| io_error(&temporary, error))?;
        copied += size as u64;
        progress(copied);
    }
    output.sync_all().map_err(|error| io_error(&temporary, error))?;
    drop(output);
    if !same(&source, &temporary)? {
        return Err(invalid(&temporary, "copied cache file failed verification"));
    }
    checked_parent(&plan.destination, relative, true)?;
    if destination.exists() {
        return Err(invalid(&destination, "destination appeared during migration"));
    }
    fs::rename(&temporary, &destination).map_err(|error| io_error(&destination, error))
}

fn cleanup(plan: &CacheMigration, files: &[(PathBuf, u64)]) -> Result<(), StoreError> {
    for (relative, _) in files {
        let source = plan.source.join(relative);
        if !same(&source, &plan.destination.join(relative))? {
            return Err(invalid(&source, "source changed after the cache copy"));
        }
        fs::remove_file(&source).map_err(|error| io_error(&source, error))?;
    }
    remove_stage(plan)
}

fn checked_parent(root: &Path, relative: &Path, create: bool) -> Result<(), StoreError> {
    let mut path = root.to_path_buf();
    for part in relative.parent().into_iter().flat_map(Path::components) {
        let std::path::Component::Normal(part) = part else {
            return Err(invalid(relative, "invalid cache entry path"));
        };
        path.push(part);
        if !path.try_exists().map_err(|error| io_error(&path, error))? {
            if !create {
                return Ok(());
            }
            fs::create_dir(&path).map_err(|error| io_error(&path, error))?;
        }
        if !regular(&path)?.is_dir() {
            return Err(invalid(&path, "cache parent is not a directory"));
        }
    }
    Ok(())
}

fn stage_root(plan: &CacheMigration) -> PathBuf {
    plan.destination.join(format!(".folia-cache-migration-{}", plan.staging))
}

fn remove_stage(plan: &CacheMigration) -> Result<(), StoreError> {
    let root = stage_root(plan);
    if root.try_exists().map_err(|error| io_error(&root, error))? {
        remove_owned(&root)?;
    }
    Ok(())
}

fn remove_owned(path: &Path) -> Result<(), StoreError> {
    if regular(path)?.is_dir() {
        for entry in fs::read_dir(path).map_err(|error| io_error(path, error))? {
            remove_owned(&entry.map_err(|error| io_error(path, error))?.path())?;
        }
        fs::remove_dir(path).map_err(|error| io_error(path, error))
    } else {
        fs::remove_file(path).map_err(|error| io_error(path, error))
    }
}

fn io_error(path: &Path, source: io::Error) -> StoreError {
    StoreError::Io { path: path.into(), source }
}
fn invalid(path: &Path, message: &'static str) -> StoreError {
    io_error(path, io::Error::new(io::ErrorKind::InvalidInput, message))
}

fn transaction(store: &Store) -> Result<File, StoreError> {
    fs::create_dir_all(store.dir()).map_err(|error| io_error(store.dir(), error))?;
    let path = store.dir().join("cache-migration.lock");
    if path.exists() {
        regular(&path)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| io_error(&path, error))?;
    file.try_lock().map_err(|error| io_error(&path, error.into()))?;
    Ok(file)
}
