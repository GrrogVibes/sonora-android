use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Context as _, Result};
use rusqlite::{OptionalExtension as _, params};

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS snapshots (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        saved_at INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS local_files (
        path TEXT PRIMARY KEY,
        parent TEXT NOT NULL,
        mtime INTEGER NOT NULL,
        size INTEGER NOT NULL,
        track TEXT NOT NULL
    );
    DROP TABLE IF EXISTS local_folders;
    CREATE TABLE IF NOT EXISTS local_dirs (
        path TEXT PRIMARY KEY,
        parent TEXT NOT NULL,
        mtime INTEGER NOT NULL,
        cover TEXT,
        portrait TEXT
    );
    CREATE TABLE IF NOT EXISTS lyrics (
        key TEXT PRIMARY KEY,
        version INTEGER NOT NULL,
        stored INTEGER NOT NULL,
        value TEXT NOT NULL
    );";

/// A key to text store for data the app can always fetch again. It lives in the cache directory
/// rather than in [`crate::Database`], so deleting it costs a reload and never a favorite, a play
/// or a playlist. Every call opens its own connection, which is what makes it safe to use from a
/// blocking thread.
#[derive(Clone)]
pub struct Cache {
    path: PathBuf,
    ready: Arc<Mutex<bool>>,
}

impl Cache {
    /// `$XDG_CACHE_HOME/sonora/cache.sqlite` and equivalent on Windows.
    pub fn standard() -> Self {
        let dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("sonora");
        Self::at(dir.join("cache.sqlite"))
    }

    pub fn at(path: PathBuf) -> Self {
        Self {
            path,
            ready: Arc::new(Mutex::new(false)),
        }
    }

    /// The value under `key` as `read` makes of it. `read` sees the text where sqlite holds it,
    /// so a snapshot of many megabytes is never copied before it is parsed.
    pub fn read<T>(&self, key: &str, read: impl FnOnce(&str) -> T) -> Result<Option<T>> {
        let connection = self.open()?;
        connection
            .query_row(
                "SELECT value FROM snapshots WHERE key = ?",
                params![key],
                |row| Ok(read(row.get_ref(0)?.as_str()?)),
            )
            .optional()
            .context("cannot read a cached value")
    }

    pub fn write(&self, key: &str, value: &str, saved_at: i64) -> Result<()> {
        let connection = self.open()?;
        connection
            .execute(
                "INSERT INTO snapshots (key, value, saved_at) VALUES (?, ?, ?)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, saved_at = excluded.saved_at",
                params![key, value, saved_at],
            )
            .context("cannot write a cached value")?;
        Ok(())
    }

    /// Drops every key under `prefix`, which is how a whole provider is forgotten at once.
    pub fn forget(&self, prefix: &str) -> Result<()> {
        let connection = self.open()?;
        connection
            .execute(
                "DELETE FROM snapshots WHERE key LIKE ? || '%'",
                params![prefix],
            )
            .context("cannot drop cached values")?;
        Ok(())
    }

    /// The lyrics stored under `key` by format `version`, as the json they were written in.
    pub fn lyrics(&self, key: &str, version: u32) -> Result<Option<String>> {
        let connection = self.open()?;
        connection
            .query_row(
                "SELECT value FROM lyrics WHERE key = ? AND version = ?",
                params![key, version],
                |row| row.get(0),
            )
            .optional()
            .context("cannot read cached lyrics")
    }

    /// Stores `entries` as `(key, stored_at, json)` in one transaction, then drops all but the
    /// `keep` most recently stored. An entry never replaces one stored after it, so an old
    /// answer moved in late cannot overwrite a newer one.
    pub fn keep_lyrics(
        &self,
        version: u32,
        entries: impl IntoIterator<Item = (String, i64, String)>,
        keep: usize,
    ) -> Result<()> {
        let mut connection = self.open()?;
        let transaction = connection
            .transaction()
            .context("cannot start writing lyrics")?;
        {
            let mut insert = transaction
                .prepare(
                    "INSERT INTO lyrics (key, version, stored, value) VALUES (?, ?, ?, ?)
                     ON CONFLICT(key) DO UPDATE SET version = excluded.version,
                         stored = excluded.stored, value = excluded.value
                     WHERE excluded.stored >= lyrics.stored",
                )
                .context("cannot prepare a lyrics write")?;
            for (key, stored, value) in entries {
                insert
                    .execute(params![key, version, stored, value])
                    .context("cannot write cached lyrics")?;
            }
        }
        transaction
            .execute(
                "DELETE FROM lyrics WHERE key NOT IN
                     (SELECT key FROM lyrics ORDER BY stored DESC LIMIT ?)",
                params![keep as i64],
            )
            .context("cannot trim cached lyrics")?;
        transaction.commit().context("cannot save cached lyrics")
    }
    /// The cache is refetchable by definition (see the module docs). A database that cannot
    /// be prepared — a truncated file, a bad header, or one left half-written by a full disk —
    /// must never keep the app from starting, so move it aside and open a fresh one once.
    /// The main [`crate::Database`] deliberately has no such recovery: it holds favorites and
    /// playlists that cannot be fetched again.
    pub fn open(&self) -> Result<rusqlite::Connection> {
        match super::connect(&self.path, SCHEMA, &self.ready, "the cache") {
            Ok(connection) => Ok(connection),
            Err(error) => {
                if self.path.exists() {
                    let quarantine = self.path.with_extension("sqlite.bad");
                    let _ = std::fs::remove_file(&quarantine);
                    match std::fs::rename(&self.path, &quarantine) {
                        Ok(()) => {
                            log::warn!(
                                "cache: quarantined an invalid {} as {}; refetching",
                                self.path.display(),
                                quarantine.display()
                            );
                            *self
                                .ready
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner) = false;
                        }
                        Err(rename) => {
                            log::warn!("cache: cannot quarantine {}: {rename}", self.path.display())
                        }
                    }
                }
                super::connect(&self.path, SCHEMA, &self.ready, "the cache")
                    .with_context(|| format!("cannot recover the cache: {error:#}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_corrupt_cache_file_does_not_stop_startup() {
        let dir = std::env::temp_dir().join(format!(
            "sonora-cache-recovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cache.sqlite");
        // A truncated or otherwise invalid database: `Connection::open` succeeds, but the
        // first statement fails with "file is not a database".
        std::fs::write(&path, b"not a sqlite database at all").unwrap();

        let cache = Cache::at(path.clone());
        assert!(
            cache.open().is_ok(),
            "a bad cache file must never abort startup"
        );
        assert!(
            cache
                .read("anything", |text| text.to_owned())
                .unwrap()
                .is_none(),
            "the recovered cache must be usable"
        );
        assert!(path.exists(), "a fresh cache should have been created");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
