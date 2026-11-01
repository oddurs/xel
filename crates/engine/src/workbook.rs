use std::path::Path;

use rusqlite::{Connection, ErrorCode, OpenFlags, OptionalExtension};

use crate::error::{Error, Result};

/// The on-disk format this engine writes. Bump it with a migration whenever the
/// `xel_*` schema changes.
pub const FORMAT_VERSION: u32 = 1;

/// Lets `file(1)` and other tools identify a .xel file without opening it ("XEL1").
const APPLICATION_ID: i32 = 0x5845_4C31;

/// An open .xel file: a SQLite database with xel's metadata tables.
pub struct Workbook {
    conn: Connection,
}

impl Workbook {
    /// Creates a new, empty workbook. Fails if `path` already exists.
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            return Err(Error::AlreadyExists(path.to_path_buf()));
        }
        let mut conn = Connection::open(path)?;
        let tx = conn.transaction()?;
        tx.pragma_update(None, "application_id", APPLICATION_ID)?;
        tx.execute_batch(
            "CREATE TABLE xel_meta (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL) STRICT;",
        )?;
        tx.execute(
            "INSERT INTO xel_meta (key, value) VALUES ('format_version', ?1), ('created_by', ?2)",
            (FORMAT_VERSION.to_string(), engine_version()),
        )?;
        tx.commit()?;
        Self::configure(conn)
    }

    /// Opens an existing workbook, refusing files from a newer format.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let not_a_workbook = || Error::NotAWorkbook(path.to_path_buf());
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| match e.sqlite_error_code() {
            Some(ErrorCode::CannotOpen) => not_a_workbook(),
            _ => Error::Sqlite(e),
        })?;

        // SQLite reads the header lazily, so a non-database file only fails here.
        let version: Option<String> = match conn
            .query_row(
                "SELECT value FROM xel_meta WHERE key = 'format_version'",
                [],
                |r| r.get(0),
            )
            .optional()
        {
            Ok(v) => v,
            Err(e)
                if matches!(e.sqlite_error_code(), Some(ErrorCode::NotADatabase))
                    || is_missing_table(&e) =>
            {
                return Err(not_a_workbook());
            }
            Err(e) => return Err(e.into()),
        };
        let version = version.ok_or_else(|| Error::CorruptMetadata("no format_version".into()))?;
        let found: u32 = version
            .parse()
            .map_err(|_| Error::CorruptMetadata(format!("format_version is {version:?}")))?;
        if found > FORMAT_VERSION {
            return Err(Error::NewerFormat {
                found,
                supported: FORMAT_VERSION,
            });
        }
        Self::configure(conn)
    }

    /// The format version recorded in the file.
    pub fn format_version(&self) -> Result<u32> {
        let v: String = self.conn.query_row(
            "SELECT value FROM xel_meta WHERE key = 'format_version'",
            [],
            |r| r.get(0),
        )?;
        v.parse()
            .map_err(|_| Error::CorruptMetadata(format!("format_version is {v:?}")))
    }

    fn configure(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        Ok(Self { conn })
    }
}

fn engine_version() -> String {
    format!("xel {}", env!("CARGO_PKG_VERSION"))
}

fn is_missing_table(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::SqliteFailure(_, Some(msg)) if msg.starts_with("no such table"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn create_writes_metadata_that_plain_sqlite_can_read() {
        let dir = TempDir::new();
        let path = dir.path().join("book.xel");
        Workbook::create(&path).unwrap();

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
        let path = dir.path().join("book.xel");
        drop(Workbook::create(&path).unwrap());
        assert_eq!(
            Workbook::open(&path).unwrap().format_version().unwrap(),
            FORMAT_VERSION
        );
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
    fn newer_format_is_refused_with_both_versions() {
        let dir = TempDir::new();
        let path = dir.path().join("book.xel");
        drop(Workbook::create(&path).unwrap());
        Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE xel_meta SET value = '99' WHERE key = 'format_version'",
                [],
            )
            .unwrap();

        let err = Workbook::open(&path).err().unwrap();
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
    }

    #[test]
    fn non_sqlite_files_are_not_workbooks() {
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
    }

    #[test]
    fn plain_sqlite_databases_are_not_workbooks() {
        let dir = TempDir::new();
        let path = dir.path().join("other.db");
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE t (x)")
            .unwrap();
        assert!(matches!(Workbook::open(&path), Err(Error::NotAWorkbook(_))));
    }

    #[test]
    fn missing_files_are_not_created_by_open() {
        let dir = TempDir::new();
        let path = dir.path().join("missing.xel");
        assert!(matches!(Workbook::open(&path), Err(Error::NotAWorkbook(_))));
        assert!(!path.exists());
    }

    #[test]
    fn corrupt_version_is_reported() {
        let dir = TempDir::new();
        let path = dir.path().join("book.xel");
        drop(Workbook::create(&path).unwrap());
        Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE xel_meta SET value = 'banana' WHERE key = 'format_version'",
                [],
            )
            .unwrap();
        assert!(matches!(
            Workbook::open(&path),
            Err(Error::CorruptMetadata(_))
        ));
    }
}
