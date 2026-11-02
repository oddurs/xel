use std::fs::OpenOptions;
use std::io::ErrorKind as IoErrorKind;
use std::path::{Path, PathBuf};

use rusqlite::types::Value;
use rusqlite::{Connection, ErrorCode, MAIN_DB, OpenFlags};

use crate::error::{Error, Result};

/// The on-disk format this engine writes. Bump it with a migration whenever the
/// `xel_*` schema changes, starting with the first release; until then only development
/// builds have written files.
pub const FORMAT_VERSION: u32 = 1;

/// Lets `file(1)` and `open` identify a .xel file from its header ("XEL1").
const APPLICATION_ID: i32 = 0x5845_4C31;

/// Never `SQLITE_OPEN_CREATE`: files are created only by `create_new` below. URIs are
/// handled by [`sqlite_path`], because the bundled SQLite parses them regardless of flags.
const READ_WRITE: OpenFlags =
    OpenFlags::SQLITE_OPEN_READ_WRITE.union(OpenFlags::SQLITE_OPEN_NO_MUTEX);
const READ_ONLY: OpenFlags =
    OpenFlags::SQLITE_OPEN_READ_ONLY.union(OpenFlags::SQLITE_OPEN_NO_MUTEX);

/// An open .xel file: a SQLite database with xel's metadata tables.
#[derive(Debug)]
pub struct Workbook {
    pub(crate) conn: Connection,
    format_version: u32,
    pub(crate) history: crate::history::History,
}

impl Workbook {
    /// Creates a new, empty workbook. Fails if anything already exists at `path`.
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let path = &sqlite_path(path.as_ref())?;
        // create_new is atomic (O_EXCL), so two creators cannot both succeed, and it
        // refuses symlinks. SQLite treats the empty file as an empty database.
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| {
                if e.kind() == IoErrorKind::AlreadyExists {
                    Error::AlreadyExists(path.clone())
                } else {
                    Error::Io {
                        path: path.clone(),
                        source: e,
                    }
                }
            })?;
        Self::initialize(path).inspect_err(|_| {
            // The file is ours and half-made; leaving it would make every retry fail. The
            // initialization error is the one to report. If removal also fails, the next
            // create says AlreadyExists with the path, so the leftover is not hidden.
            for leftover in [path.clone(), sidecar(path, "-journal")] {
                let _ = std::fs::remove_file(&leftover);
            }
        })
    }

    fn initialize(path: &Path) -> Result<Self> {
        let mut conn = Connection::open_with_flags(path, READ_WRITE)?;
        let tx = conn.transaction()?;
        tx.pragma_update(None, "application_id", APPLICATION_ID)?;
        tx.execute_batch(
            "CREATE TABLE xel_meta (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL) STRICT;",
        )?;
        tx.execute_batch(crate::schema::SCHEMA)?;
        tx.execute(
            "INSERT INTO xel_meta (key, value) VALUES ('format_version', ?1), ('created_by', ?2)",
            (
                FORMAT_VERSION.to_string(),
                concat!("xel ", env!("CARGO_PKG_VERSION")),
            ),
        )?;
        tx.commit()?;
        Self::configure(conn, FORMAT_VERSION)
    }

    /// Opens an existing workbook for editing.
    ///
    /// Everything is checked on a read-only connection first, so a file that is refused
    /// (newer format, another application's database) is never written to, not even by a
    /// WAL checkpoint on close. The one exception is a crash-recovery rollback, which only
    /// restores the file's last committed state.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = &sqlite_path(path.as_ref())?;
        let checked = probe(path)?.map(|(_, version)| version);

        let conn = Connection::open_with_flags(path, READ_WRITE)?;
        // SQLite silently falls back to read-only for write-protected files; edits would
        // then fail one by one, so refuse up front.
        if conn.is_readonly(MAIN_DB)? {
            return Err(Error::ReadOnly(path.clone()));
        }
        let format_version = match checked {
            Some(version) => version,
            None => identify(&conn, path)?,
        };
        Self::configure(conn, format_version)
    }

    /// Opens `src` and makes a private working copy of it at `working`, which must not
    /// exist yet. The returned workbook edits the copy; the macOS document layer leaves the
    /// user's file untouched until it saves with [`save_copy`](Self::save_copy), so
    /// autosave, Versions, and iCloud only ever see complete files.
    ///
    /// The original is only read, so a WAL left by another tool is not checkpointed into
    /// it. The exception is a crash journal, which needs a writer to roll it back, as a
    /// byte-for-byte copy would not; a write-protected original with one is `ReadOnly`.
    pub fn open_copy(src: impl AsRef<Path>, working: impl AsRef<Path>) -> Result<Self> {
        let src = &sqlite_path(src.as_ref())?;
        let working = &sqlite_path(working.as_ref())?;
        if let Some((original, _)) = probe(src)? {
            write_copy(&original, working)?;
        } else {
            let original = Self::open(src)?;
            write_copy(&original.conn, working)?;
            original.close().inspect_err(|_| remove_partial(working))?;
        }
        Self::open(working).inspect_err(|_| remove_partial(working))
    }

    /// Writes a complete, synced copy of this workbook to `dest`, which must not exist yet,
    /// without the undo trash this session keeps. The document layer writes to a fresh
    /// temporary file and moves it into place, so a crash part-way leaves the user's file
    /// as it was.
    pub fn save_copy(&self, dest: impl AsRef<Path>) -> Result<()> {
        write_copy(&self.conn, dest.as_ref())
    }

    /// The format version recorded in the file when it was opened.
    pub fn format_version(&self) -> u32 {
        self.format_version
    }

    /// Closes the file, reporting failures that dropping the workbook would hide. Undo
    /// history ends here, so the data it was keeping for deleted tables and columns goes.
    pub fn close(self) -> Result<()> {
        self.purge_trash()?;
        self.conn.close().map_err(|(_, e)| e.into())
    }

    fn configure(conn: Connection, format_version: u32) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        // SQLite treats an unknown "name" as a string literal. A column dropped by another
        // tool would then read back as its own name in every row; make it an error.
        conn.set_db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_DQS_DML, false)?;
        conn.set_db_config(rusqlite::config::DbConfig::SQLITE_DBCONFIG_DQS_DDL, false)?;
        // Another reader (sqlite3, a Python script) may briefly hold the file; wait for it
        // rather than failing the user's edit.
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(Self {
            conn,
            format_version,
            history: crate::history::History::new(),
        })
    }
}

/// Checks that `conn` is an xel workbook this engine can open and returns its version.
fn identify(conn: &Connection, path: &Path) -> Result<u32> {
    // Reading the header is the first access, so non-database bytes fail here.
    let app_id: i32 = match conn.pragma_query_value(None, "application_id", |r| r.get(0)) {
        Ok(id) => id,
        Err(e) if matches!(e.sqlite_error_code(), Some(ErrorCode::NotADatabase)) => {
            return Err(Error::NotAWorkbook(path.to_path_buf()));
        }
        Err(e) if matches!(e.sqlite_error_code(), Some(ErrorCode::DatabaseCorrupt)) => {
            return Err(Error::Corrupt(e.to_string()));
        }
        Err(e) => return Err(e.into()),
    };
    if app_id != APPLICATION_ID {
        return Err(Error::NotAWorkbook(path.to_path_buf()));
    }

    // From here the file claims to be ours, so anything unexpected means damage.
    let raw: Value = conn
        .query_row(
            "SELECT value FROM xel_meta WHERE key = 'format_version'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                Error::Corrupt("format_version is missing".into())
            }
            e if is_hot_journal(&e) => Error::Sqlite(e),
            e => Error::Corrupt(e.to_string()),
        })?;
    let found: u64 = match raw {
        Value::Text(s) if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => {
            // All digits but too large for u64 is still "newer than us".
            s.parse().unwrap_or(u64::MAX)
        }
        Value::Integer(n) if n >= 0 => n.unsigned_abs(),
        other => return Err(Error::Corrupt(format!("format_version is {other:?}"))),
    };
    match u32::try_from(found) {
        Ok(v) if v <= FORMAT_VERSION => Ok(v),
        _ => Err(Error::NewerFormat {
            found,
            supported: FORMAT_VERSION,
        }),
    }
}

/// Checks `path` on a read-only connection, which never writes to the file. Returns the
/// connection and format version, or `None` when a crash journal needs a writer to roll
/// it back first.
fn probe(path: &Path) -> Result<Option<(Connection, u32)>> {
    let io = |e: std::io::Error| match e.kind() {
        IoErrorKind::NotFound => Error::NotFound(path.to_path_buf()),
        _ => Error::Io {
            path: path.to_path_buf(),
            source: e,
        },
    };
    if std::fs::metadata(path).map_err(io)?.is_dir() {
        return Err(Error::NotAWorkbook(path.to_path_buf()));
    }
    let probed = Connection::open_with_flags(path, READ_ONLY)
        .map_err(|e| explain_cannot_open(e, path))
        .and_then(|conn| identify(&conn, path).map(|version| (conn, version)));
    match probed {
        Ok(found) => Ok(Some(found)),
        Err(Error::Sqlite(e)) if is_hot_journal(&e) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Writes a compacted copy of `conn`'s database to `dest`, which must not exist yet.
fn write_copy(conn: &Connection, dest: &Path) -> Result<()> {
    let dest = &sqlite_path(dest)?;
    let io = |e: std::io::Error| match e.kind() {
        IoErrorKind::AlreadyExists => Error::AlreadyExists(dest.clone()),
        _ => Error::Io {
            path: dest.clone(),
            source: e,
        },
    };
    let name = dest.to_str().ok_or_else(|| {
        io(std::io::Error::new(
            IoErrorKind::InvalidInput,
            "SQLite needs a UTF-8 path",
        ))
    })?;
    // SQLite would treat a journal already beside the name as its own and delete it.
    if sidecar(dest, "-journal").exists() {
        return Err(Error::AlreadyExists(sidecar(dest, "-journal")));
    }
    // Creating the file first claims the name, so a failure below removes only our files.
    // The bundled SQLite's VACUUM INTO accepts an empty file; older ones refuse it.
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
        .map_err(io)?;
    let written = conn
        .execute("VACUUM INTO ?1", [name])
        .map_err(Error::from)
        .and_then(|_| drop_trash(dest))
        // The copy must be on disk before it replaces the user's file. SQLite's fsync
        // leaves it in the drive's cache on macOS; sync_all is F_FULLFSYNC.
        .and_then(|()| file.sync_all().map_err(io));
    written.inspect_err(|_| remove_partial(dest))
}

/// Removes a copy this module created and then failed to finish, so it cannot pass for a
/// finished one.
fn remove_partial(path: &Path) {
    // The failure that led here is the error to report; one in removing the leftovers as
    // well changes nothing the caller can act on. They are files we created, in the
    // document layer's temporary directory.
    let _ = std::fs::remove_file(sidecar(path, "-journal"));
    let _ = std::fs::remove_file(path);
}

/// Removes undo trash from a fresh copy: trash is a session's undo state, not part of the
/// document.
fn drop_trash(copy: &Path) -> Result<()> {
    let conn = Connection::open_with_flags(copy, READ_WRITE)?;
    // Zeroing the freed pages keeps deleted data out of the saved file without a second
    // full copy.
    conn.pragma_update(None, "secure_delete", true)?;
    if crate::history::drop_all_trash(&conn)? {
        let free: i64 = conn.pragma_query_value(None, "freelist_count", |r| r.get(0))?;
        let pages: i64 = conn.pragma_query_value(None, "page_count", |r| r.get(0))?;
        // Compacting copies the file again; only worth it when the trash was a large
        // part of it, as after deleting a big table.
        if free * 4 > pages {
            conn.execute_batch("VACUUM;")?;
        }
    }
    conn.close().map_err(|(_, e)| e.into())
}

/// SQLite reports an unreadable file only as "unable to open". Asking the OS says why
/// (permission denied). This runs only after SQLite has failed to open the file, so no
/// connection in this process holds locks on it: opening and closing another descriptor
/// would otherwise release them, as POSIX locks are per process.
fn explain_cannot_open(e: rusqlite::Error, path: &Path) -> Error {
    if matches!(e.sqlite_error_code(), Some(ErrorCode::CannotOpen))
        && let Err(io) = std::fs::File::open(path)
    {
        return Error::Io {
            path: path.to_path_buf(),
            source: io,
        };
    }
    e.into()
}

/// An absolute path for SQLite. The bundled build always parses `file:` URIs, so a
/// relative path such as `file:other.db` would open `other.db`; an absolute path never
/// starts with `file:`.
fn sqlite_path(path: &Path) -> Result<PathBuf> {
    std::path::absolute(path).map_err(|e| Error::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

fn is_hot_journal(e: &rusqlite::Error) -> bool {
    // SQLITE_READONLY_ROLLBACK: a read-only connection found a journal it must roll back.
    const READONLY_ROLLBACK: std::ffi::c_int = 776;
    matches!(e, rusqlite::Error::SqliteFailure(f, _) if f.extended_code == READONLY_ROLLBACK)
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    name.into()
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::*;
    use crate::test_support::TempDir;
    use crate::{CellValue, ColumnType};

    fn created(dir: &TempDir) -> PathBuf {
        let path = dir.path().join("book.xel");
        drop(Workbook::create(&path).unwrap());
        path
    }

    fn set_version(path: &Path, sql_value: &str) {
        Connection::open(path)
            .unwrap()
            .execute(
                &format!("UPDATE xel_meta SET value = {sql_value} WHERE key = 'format_version'"),
                [],
            )
            .unwrap();
    }

    #[test]
    fn create_writes_metadata_that_plain_sqlite_can_read() {
        let dir = TempDir::new();
        let path = created(&dir);
        let raw = Connection::open(&path).unwrap();
        let version: String = raw
            .query_row(
                "SELECT value FROM xel_meta WHERE key = 'format_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(version, FORMAT_VERSION.to_string());
        let app_id: i32 = raw
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .unwrap();
        assert_eq!(app_id, APPLICATION_ID);
    }

    #[test]
    fn reopens_what_it_created() {
        let dir = TempDir::new();
        let book = Workbook::open(created(&dir)).unwrap();
        assert_eq!(book.format_version(), FORMAT_VERSION);
        book.close().unwrap();
    }

    #[test]
    fn create_refuses_to_overwrite() {
        let dir = TempDir::new();
        let path = dir.path().join("book.xel");
        std::fs::write(&path, "keep me").unwrap();
        assert!(matches!(
            Workbook::create(&path),
            Err(Error::AlreadyExists(_))
        ));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep me");
    }

    #[test]
    fn relative_uri_like_paths_reach_sqlite_as_absolute_paths() {
        let p = sqlite_path(Path::new("file:other.db?mode=memory")).unwrap();
        assert!(p.is_absolute());
        assert!(p.ends_with("file:other.db?mode=memory"));
    }

    #[test]
    fn create_takes_uri_like_paths_literally() {
        let dir = TempDir::new();
        let other = dir.path().join("other.db");
        Connection::open(&other)
            .unwrap()
            .execute_batch("CREATE TABLE t (x)")
            .unwrap();
        let before = std::fs::read(&other).unwrap();

        let literal = dir.path().join("file:other.db");
        Workbook::create(&literal).unwrap();
        assert!(literal.exists());
        assert_eq!(
            std::fs::read(&other).unwrap(),
            before,
            "other.db must be untouched"
        );
    }

    #[test]
    fn failed_create_leaves_nothing_behind() {
        let dir = TempDir::new();
        let path = dir.path().join("blocked.xel");
        // A directory where SQLite wants its rollback journal makes the commit fail.
        std::fs::create_dir(sidecar(&path, "-journal")).unwrap();
        assert!(Workbook::create(&path).is_err());
        assert!(!path.exists(), "half-made file was left behind");
    }

    #[test]
    fn create_does_not_follow_symlinks() {
        let dir = TempDir::new();
        let target = dir.path().join("target.xel");
        let link = dir.path().join("link.xel");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(matches!(
            Workbook::create(&link),
            Err(Error::AlreadyExists(_))
        ));
        assert!(!target.exists());
    }

    #[test]
    fn newer_format_is_refused_with_both_versions() {
        let dir = TempDir::new();
        let path = created(&dir);
        set_version(&path, "'99'");
        let err = Workbook::open(&path).unwrap_err();
        assert!(matches!(
            err,
            Error::NewerFormat {
                found: 99,
                supported: FORMAT_VERSION
            }
        ));
        let msg = err.to_string();
        assert!(
            msg.contains("99") && msg.contains(&FORMAT_VERSION.to_string()),
            "{msg}"
        );

        set_version(&path, "'4294967296'");
        assert!(matches!(
            Workbook::open(&path),
            Err(Error::NewerFormat {
                found: 4_294_967_296,
                ..
            })
        ));
    }

    #[test]
    fn refusing_a_newer_file_does_not_write_to_it() {
        let dir = TempDir::new();
        let path = created(&dir);
        // Leave committed frames in the WAL, as a newer xel that crashed would.
        let writer = Connection::open(&path).unwrap();
        writer.pragma_update(None, "journal_mode", "WAL").unwrap();
        writer.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
        writer
            .execute(
                "UPDATE xel_meta SET value = '2' WHERE key = 'format_version'",
                [],
            )
            .unwrap();
        let wal = sidecar(&path, "-wal");
        let main_before = std::fs::read(&path).unwrap();
        let wal_before = std::fs::read(&wal).unwrap();
        assert!(!wal_before.is_empty());
        // Keep the writer open but unused, like the crashed process.
        std::mem::forget(writer);

        assert!(matches!(
            Workbook::open(&path),
            Err(Error::NewerFormat { found: 2, .. })
        ));
        assert_eq!(
            std::fs::read(&path).unwrap(),
            main_before,
            "main file was written"
        );
        assert_eq!(
            std::fs::read(&wal).unwrap(),
            wal_before,
            "WAL was checkpointed"
        );
    }

    #[test]
    fn a_crash_mid_write_is_recovered_on_open() {
        let dir = TempDir::new();
        let path = created(&dir);
        let writer = Connection::open(&path).unwrap();
        // A tiny cache forces pages out to the file before commit, so the journal matters.
        writer.pragma_update(None, "cache_size", 1).unwrap();
        writer
            .execute_batch("BEGIN; CREATE TABLE big (x);")
            .unwrap();
        for _ in 0..200 {
            writer
                .execute("INSERT INTO big VALUES (randomblob(4000))", [])
                .unwrap();
        }
        // Copy the file and its journal mid-transaction: the copy is what a crash leaves.
        let crashed = dir.path().join("crashed.xel");
        std::fs::copy(&path, &crashed).unwrap();
        std::fs::copy(sidecar(&path, "-journal"), sidecar(&crashed, "-journal")).unwrap();
        drop(writer);

        let book = Workbook::open(&crashed).unwrap();
        assert_eq!(book.format_version(), FORMAT_VERSION);
        book.close().unwrap();
        let tables: i64 = Connection::open(&crashed)
            .unwrap()
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'big'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 0, "the uncommitted transaction was rolled back");
    }

    #[test]
    fn non_database_files_are_not_workbooks() {
        let dir = TempDir::new();
        for (name, contents) in [
            ("text.xel", b"hello, world".as_slice()),
            ("empty.xel", b"".as_slice()),
            ("binary.xel", &[0xFF; 4096]),
        ] {
            let path = dir.path().join(name);
            std::fs::write(&path, contents).unwrap();
            assert!(
                matches!(Workbook::open(&path), Err(Error::NotAWorkbook(_))),
                "{name} should not open"
            );
        }
        assert!(matches!(
            Workbook::open(dir.path()),
            Err(Error::NotAWorkbook(_))
        ));
    }

    #[test]
    fn other_sqlite_databases_are_not_workbooks() {
        let dir = TempDir::new();
        let path = dir.path().join("other.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE xel_meta (key, value); \
                 INSERT INTO xel_meta VALUES ('format_version', '1');",
            )
            .unwrap();
        assert!(
            matches!(Workbook::open(&path), Err(Error::NotAWorkbook(_))),
            "an xel_meta table alone does not make a workbook; the application id does"
        );
    }

    #[test]
    fn damaged_metadata_is_corrupt_not_a_database_error() {
        let dir = TempDir::new();
        for value in ["'banana'", "'+1'", "''", "NULL", "-1", "1.5"] {
            let path = created(&dir);
            let raw = Connection::open(&path).unwrap();
            // Rebuild without STRICT so non-text values can be stored, as a damaged or
            // hand-edited file might contain.
            raw.execute_batch(&format!(
                "DROP TABLE xel_meta; CREATE TABLE xel_meta (key, value); \
                 INSERT INTO xel_meta VALUES ('format_version', {value});"
            ))
            .unwrap();
            drop(raw);
            assert!(
                matches!(Workbook::open(&path), Err(Error::Corrupt(_))),
                "{value}"
            );
            std::fs::remove_file(&path).unwrap();
        }

        let path = created(&dir);
        Connection::open(&path)
            .unwrap()
            .execute_batch("DROP TABLE xel_meta; CREATE TABLE xel_meta (a, b);")
            .unwrap();
        assert!(matches!(Workbook::open(&path), Err(Error::Corrupt(_))));
        std::fs::remove_file(&path).unwrap();

        let path = created(&dir);
        Connection::open(&path)
            .unwrap()
            .execute("DELETE FROM xel_meta", [])
            .unwrap();
        let err = Workbook::open(&path).unwrap_err();
        assert!(
            err.to_string().contains("format_version is missing"),
            "{err}"
        );
    }

    #[test]
    fn integer_versions_are_read() {
        let dir = TempDir::new();
        let path = created(&dir);
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "DROP TABLE xel_meta; CREATE TABLE xel_meta (key, value); \
                 INSERT INTO xel_meta VALUES ('format_version', 2);",
            )
            .unwrap();
        assert!(matches!(
            Workbook::open(&path),
            Err(Error::NewerFormat { found: 2, .. })
        ));
    }

    #[test]
    fn truncated_files_are_corrupt_or_not_workbooks() {
        let dir = TempDir::new();
        let path = created(&dir);
        let bytes = std::fs::read(&path).unwrap();
        // A partial last page is noticed only when that page is read; a full check is the
        // v1.0 crash-safety suite's job.
        for len in [50, 100] {
            std::fs::write(&path, &bytes[..len]).unwrap();
            let err = Workbook::open(&path).unwrap_err();
            assert!(
                matches!(err, Error::Corrupt(_) | Error::NotAWorkbook(_)),
                "{len}: {err:?}"
            );
        }
    }

    #[test]
    fn missing_files_are_reported_as_missing_and_not_created() {
        let dir = TempDir::new();
        let path = dir.path().join("missing.xel");
        assert!(matches!(Workbook::open(&path), Err(Error::NotFound(_))));
        assert!(!path.exists());
    }

    #[test]
    fn unreadable_and_read_only_files_say_so() {
        let dir = TempDir::new();
        let path = created(&dir);
        let set = |mode| std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode));

        set(0o444).unwrap();
        assert!(matches!(Workbook::open(&path), Err(Error::ReadOnly(_))));

        set(0o000).unwrap();
        let err = Workbook::open(&path).unwrap_err();
        set(0o644).unwrap();
        assert!(
            matches!(&err, Error::Io { source, .. } if source.kind() == IoErrorKind::PermissionDenied),
            "permission problem reported as {err}"
        );
    }

    fn trash_tables(path: &Path) -> i64 {
        Connection::open(path)
            .unwrap()
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name GLOB 'xel_trash_*'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    }

    fn text(s: &str) -> CellValue {
        CellValue::Text(s.into())
    }

    #[test]
    fn a_working_copy_leaves_the_original_untouched_until_saved() {
        let dir = TempDir::new();
        let doc = dir.path().join("doc.xel");
        let mut book = Workbook::create(&doc).unwrap();
        let t = book.create_table("T", &[("a", ColumnType::Text)]).unwrap();
        let a = book.table(t).unwrap().columns[0].id;
        book.insert_row(t, &[(a, text("one"))]).unwrap();
        book.close().unwrap();
        let before = std::fs::read(&doc).unwrap();

        let working = dir.path().join("working.xel");
        let mut book = Workbook::open_copy(&doc, &working).unwrap();
        book.insert_row(t, &[(a, text("two"))]).unwrap();
        assert_eq!(
            std::fs::read(&doc).unwrap(),
            before,
            "edits stay in the copy"
        );

        let saved = dir.path().join("saved.xel");
        book.save_copy(&saved).unwrap();
        std::fs::rename(&saved, &doc).unwrap();
        book.insert_row(t, &[(a, text("three"))]).unwrap();
        book.close().unwrap();
        let reopened = Workbook::open(&doc).unwrap();
        assert_eq!(reopened.row_count(t).unwrap(), 2, "the save is a snapshot");
    }

    #[test]
    fn saved_copies_leave_out_undo_trash_and_the_data_it_held() {
        let dir = TempDir::new();
        let mut book = Workbook::create(dir.path().join("doc.xel")).unwrap();
        let t = book
            .create_table(
                "T",
                &[("a", ColumnType::Text), ("secret", ColumnType::Text)],
            )
            .unwrap();
        let cols = book.table(t).unwrap().columns;
        let marker = "deleted-secret-marker";
        book.insert_row(t, &[(cols[0].id, text("kept")), (cols[1].id, text(marker))])
            .unwrap();
        // Most of the file is kept data, so the copy is not compacted again and the
        // zeroing of freed pages is what keeps the secret out.
        book.conn
            .execute_batch(
                "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 200)
                 INSERT INTO \"T\" (\"a\") SELECT hex(randomblob(500)) FROM n;",
            )
            .unwrap();
        book.delete_column(cols[1].id).unwrap();

        let saved = dir.path().join("saved.xel");
        book.save_copy(&saved).unwrap();
        assert_eq!(trash_tables(&saved), 0);
        let free: i64 = Connection::open(&saved)
            .unwrap()
            .query_row("PRAGMA freelist_count", [], |r| r.get(0))
            .unwrap();
        assert!(free > 0, "this test is meant to cover the uncompacted path");
        let bytes = std::fs::read(&saved).unwrap();
        assert!(
            !bytes.windows(marker.len()).any(|w| w == marker.as_bytes()),
            "deleted data is not left in the saved file's free pages"
        );

        book.undo().unwrap();
        assert_eq!(
            book.table(t).unwrap().columns.len(),
            2,
            "the session's own undo still works after a save"
        );
        let copy = Workbook::open(&saved).unwrap();
        assert_eq!(copy.table(t).unwrap().columns.len(), 1);
    }

    #[test]
    fn copies_never_overwrite() {
        let dir = TempDir::new();
        let doc = created(&dir);
        let taken = dir.path().join("taken.xel");
        std::fs::write(&taken, b"someone else's").unwrap();

        let book = Workbook::open(&doc).unwrap();
        assert!(matches!(
            book.save_copy(&taken),
            Err(Error::AlreadyExists(_))
        ));
        assert!(matches!(
            Workbook::open_copy(&doc, &taken),
            Err(Error::AlreadyExists(_))
        ));
        assert_eq!(std::fs::read(&taken).unwrap(), b"someone else's");

        let free = dir.path().join("free.xel");
        std::fs::write(sidecar(&free, "-journal"), b"another file's journal").unwrap();
        assert!(matches!(
            book.save_copy(&free),
            Err(Error::AlreadyExists(_))
        ));
        assert!(!free.exists());
        assert_eq!(
            std::fs::read(sidecar(&free, "-journal")).unwrap(),
            b"another file's journal"
        );
    }

    #[test]
    fn open_copy_recovers_a_crashed_original_and_reads_a_locked_one() {
        let dir = TempDir::new();
        let path = created(&dir);
        let writer = Connection::open(&path).unwrap();
        writer.pragma_update(None, "cache_size", 1).unwrap();
        writer
            .execute_batch(
                "BEGIN; CREATE TABLE big (x);
                 WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 200)
                 INSERT INTO big SELECT randomblob(4000) FROM n;",
            )
            .unwrap();
        let crashed = dir.path().join("crashed.xel");
        std::fs::copy(&path, &crashed).unwrap();
        std::fs::copy(sidecar(&path, "-journal"), sidecar(&crashed, "-journal")).unwrap();
        let locked_crash = dir.path().join("locked-crash.xel");
        std::fs::copy(&path, &locked_crash).unwrap();
        std::fs::copy(
            sidecar(&path, "-journal"),
            sidecar(&locked_crash, "-journal"),
        )
        .unwrap();
        drop(writer);

        let working = dir.path().join("working.xel");
        Workbook::open_copy(&crashed, &working)
            .unwrap()
            .close()
            .unwrap();
        let big: i64 = Connection::open(&working)
            .unwrap()
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name = 'big'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(big, 0, "the copy has the last committed state");

        std::fs::set_permissions(&locked_crash, std::fs::Permissions::from_mode(0o444)).unwrap();
        let fresh = dir.path().join("never-made.xel");
        assert!(matches!(
            Workbook::open_copy(&locked_crash, &fresh),
            Err(Error::ReadOnly(_))
        ));
        assert!(!fresh.exists());

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut book = Workbook::open_copy(&path, dir.path().join("from-locked.xel")).unwrap();
        book.create_table("T", &[("a", ColumnType::Text)]).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn open_copy_never_writes_to_the_original() {
        let dir = TempDir::new();
        let path = created(&dir);
        // Another tool crashed with the file in WAL mode and changes not yet checkpointed:
        // copy the file and its WAL while that tool still holds them.
        let other = Connection::open(&path).unwrap();
        other.pragma_update(None, "journal_mode", "wal").unwrap();
        other.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
        other.execute_batch("CREATE TABLE in_wal (x);").unwrap();
        let doc = dir.path().join("crashed-wal.xel");
        std::fs::copy(&path, &doc).unwrap();
        std::fs::copy(sidecar(&path, "-wal"), sidecar(&doc, "-wal")).unwrap();
        drop(other);
        let before = std::fs::read(&doc).unwrap();

        let working = dir.path().join("working.xel");
        let book = Workbook::open_copy(&doc, &working).unwrap();
        let copied: i64 = book
            .conn
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name = 'in_wal'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(copied, 1, "the copy sees the WAL's changes");
        book.close().unwrap();
        assert_eq!(
            std::fs::read(&doc).unwrap(),
            before,
            "nothing was checkpointed into it"
        );
        assert!(sidecar(&doc, "-wal").exists());

        assert!(matches!(
            Workbook::open_copy(dir.path().join("missing.xel"), dir.path().join("w2.xel")),
            Err(Error::NotFound(_))
        ));
        assert!(!dir.path().join("w2.xel").exists());
    }

    const SAVE_LOOP_DIR: &str = "XEL_TEST_SAVE_LOOP_DIR";

    /// The child half of `a_kill_during_save_never_damages_the_document`: edits a working
    /// copy and saves it over the document the way the app does, until it is killed.
    #[test]
    #[ignore = "run by a_kill_during_save_never_damages_the_document"]
    fn save_until_killed() {
        let Some(dir) = std::env::var_os(SAVE_LOOP_DIR).map(PathBuf::from) else {
            return;
        };
        let doc = dir.join("doc.xel");
        let mut book = Workbook::open_copy(&doc, dir.join("working.xel")).unwrap();
        let table = book.tables().unwrap().remove(0);
        // Deleted data in the undo trash makes every save take its longest path.
        let waste = book
            .create_table("Waste", &[("x", ColumnType::Text)])
            .unwrap();
        book.conn
            .execute_batch(
                "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 500)
                 INSERT INTO \"Waste\" (\"x\") SELECT hex(randomblob(2000)) FROM n;",
            )
            .unwrap();
        book.delete_table(waste).unwrap();
        // Saves until the parent kills it; nothing else ends this loop.
        let mut i = 0_u64;
        loop {
            book.insert_row(
                table.id,
                &[(table.columns[0].id, text(&format!("save {i}")))],
            )
            .unwrap();
            let saving = dir.join(format!("saving-{i}.xel"));
            book.save_copy(&saving).unwrap();
            std::fs::rename(&saving, &doc).unwrap();
            if i == 0 {
                // After one completed save, so a slow machine cannot make the kills miss.
                std::fs::write(dir.join("ready"), b"").unwrap();
            }
            i += 1;
        }
    }

    #[test]
    fn a_kill_during_save_never_damages_the_document() {
        let dir = TempDir::new();
        let doc = dir.path().join("doc.xel");
        let mut book = Workbook::create(&doc).unwrap();
        let t = book
            .create_table(
                "T",
                &[("saved", ColumnType::Text), ("filler", ColumnType::Text)],
            )
            .unwrap();
        // Enough data that a save takes milliseconds, so kills land inside one.
        book.conn
            .execute_batch(
                "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 1000)
                 INSERT INTO \"T\" (\"filler\") SELECT hex(randomblob(1000)) FROM n;",
            )
            .unwrap();
        book.close().unwrap();
        let initial = 1000;

        let exe = std::env::current_exe().unwrap();
        let ready = dir.path().join("ready");
        let (mut mid_save, mut last) = (0, initial);
        for round in 0..20_u64 {
            let mut child = Command::new(&exe)
                .args(["--exact", "workbook::tests::save_until_killed", "--ignored"])
                .env(SAVE_LOOP_DIR, dir.path())
                .stdout(Stdio::null())
                .spawn()
                .unwrap();
            let started = Instant::now();
            while !ready.exists() {
                assert!(child.try_wait().unwrap().is_none(), "the child failed");
                assert!(
                    started.elapsed() < Duration::from_secs(30),
                    "the child never started"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            // Spread the kills across the save cycles that follow.
            std::thread::sleep(Duration::from_millis(5 + round * 7 % 60));
            assert!(
                child.try_wait().unwrap().is_none(),
                "the child stopped saving"
            );
            child.kill().unwrap();
            child.wait().unwrap();

            let mut partial = false;
            for entry in std::fs::read_dir(dir.path()).unwrap() {
                let path = entry.unwrap().path();
                if path == doc {
                    continue;
                }
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                partial |= name.starts_with("saving-");
                // Clear the leftovers, as the document layer's temporary directory would be.
                std::fs::remove_file(&path).unwrap();
            }
            mid_save += u32::from(partial);

            let book = Workbook::open(&doc).unwrap();
            let rows = book.row_count(t).unwrap();
            assert!(
                rows >= last,
                "round {round}: saved rows went from {last} to {rows}"
            );
            last = rows;
            book.close().unwrap();
            let check: String = Connection::open(&doc)
                .unwrap()
                .query_row("PRAGMA integrity_check", [], |r| r.get(0))
                .unwrap();
            assert_eq!(check, "ok", "round {round}");
            assert_eq!(trash_tables(&doc), 0, "round {round}");
        }
        eprintln!(
            "20 kills: {mid_save} inside a save; {} saves completed",
            last - initial
        );
        assert!(mid_save > 0, "no kill landed inside a save");
        assert!(last > initial, "no save completed");
    }
}
