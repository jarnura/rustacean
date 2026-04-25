use std::collections::HashMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use sqlx::{Connection, PgConnection};

use crate::error::MigrateError;

#[derive(Debug, Clone)]
pub struct MigrationFile {
    pub version: i32,
    pub description: String,
    pub checksum: String,
    pub sql: String,
}

impl MigrationFile {
    pub fn load(path: &Path) -> Result<Self, MigrateError> {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| MigrateError::InvalidFilename(path.display().to_string()))?;

        let (prefix, rest) = filename
            .split_once('_')
            .ok_or_else(|| MigrateError::InvalidFilename(filename.to_string()))?;

        let version = prefix
            .parse::<i32>()
            .map_err(|_| MigrateError::InvalidFilename(filename.to_string()))?;

        let description = rest.trim_end_matches(".sql").replace('_', " ");
        let sql = std::fs::read_to_string(path)?;

        let mut hasher = Sha256::new();
        hasher.update(sql.as_bytes());
        let checksum = hex::encode(hasher.finalize());

        Ok(Self { version, description, checksum, sql })
    }
}

#[derive(Debug)]
pub struct MigrationStatus {
    pub version: i32,
    pub description: String,
    pub applied: bool,
}

pub struct Runner<'a> {
    schema: &'a str,
    dir: &'a Path,
}

impl<'a> Runner<'a> {
    pub fn new(schema: &'a str, dir: &'a Path) -> Self {
        Self { schema, dir }
    }

    pub async fn bootstrap(&self, conn: &mut PgConnection) -> Result<(), MigrateError> {
        sqlx::query(&format!(r#"CREATE SCHEMA IF NOT EXISTS "{}""#, self.schema))
            .execute(&mut *conn)
            .await?;

        sqlx::query(&format!(
            r#"CREATE TABLE IF NOT EXISTS "{}".schema_migrations (
                version     INTEGER     PRIMARY KEY,
                description TEXT        NOT NULL,
                checksum    TEXT        NOT NULL,
                applied_at  TIMESTAMPTZ NOT NULL DEFAULT now()
            )"#,
            self.schema
        ))
        .execute(&mut *conn)
        .await?;

        Ok(())
    }

    pub async fn apply_all(&self, conn: &mut PgConnection) -> Result<usize, MigrateError> {
        let mut files = self.load_files()?;
        files.sort_by_key(|f| f.version);

        let applied = self.applied_versions(conn).await?;
        let mut count = 0usize;

        for file in &files {
            match applied.get(&file.version) {
                Some(stored) if stored != &file.checksum => {
                    return Err(MigrateError::ChecksumMismatch {
                        version: file.version,
                        stored: stored.clone(),
                        actual: file.checksum.clone(),
                    });
                }
                Some(_) => {}  // already applied, checksum matches
                None => {
                    self.apply_one(conn, file).await?;
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    pub async fn status(&self, conn: &mut PgConnection) -> Result<Vec<MigrationStatus>, MigrateError> {
        let mut files = self.load_files()?;
        files.sort_by_key(|f| f.version);

        let applied = self.applied_versions(conn).await?;

        Ok(files
            .into_iter()
            .map(|f| MigrationStatus {
                applied: applied.contains_key(&f.version),
                version: f.version,
                description: f.description,
            })
            .collect())
    }

    async fn apply_one(&self, conn: &mut PgConnection, file: &MigrationFile) -> Result<(), MigrateError> {
        let mut tx = conn.begin().await?;

        // SET LOCAL is transaction-scoped; lets migration SQL omit schema prefix.
        // Runner doesn't go through PgBouncer, so search_path is safe here.
        sqlx::query(&format!(
            r#"SET LOCAL search_path TO "{}", public"#,
            self.schema
        ))
        .execute(&mut *tx)
        .await?;

        sqlx::raw_sql(&file.sql).execute(&mut *tx).await?;

        sqlx::query(&format!(
            r#"INSERT INTO "{}".schema_migrations (version, description, checksum)
               VALUES ($1, $2, $3)"#,
            self.schema
        ))
        .bind(file.version)
        .bind(&file.description)
        .bind(&file.checksum)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn applied_versions(&self, conn: &mut PgConnection) -> Result<HashMap<i32, String>, MigrateError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            version: i32,
            checksum: String,
        }

        let rows: Vec<Row> = sqlx::query_as(&format!(
            r#"SELECT version, checksum FROM "{}".schema_migrations ORDER BY version"#,
            self.schema
        ))
        .fetch_all(&mut *conn)
        .await?;

        Ok(rows.into_iter().map(|r| (r.version, r.checksum)).collect())
    }

    fn load_files(&self) -> Result<Vec<MigrationFile>, MigrateError> {
        if !self.dir.exists() {
            return Err(MigrateError::MissingDir(self.dir.display().to_string()));
        }

        let mut files = Vec::new();
        for entry in std::fs::read_dir(self.dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) == Some("sql") {
                files.push(MigrationFile::load(&path)?);
            }
        }
        Ok(files)
    }
}

/// Returns the path to the migrations directory for a given sub-dir name.
pub fn migrations_dir(base: &Path, subdir: &str) -> PathBuf {
    base.join("migrations").join(subdir)
}
