use rb_tenant::TenantCtx;
use sqlx::PgPool;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
}

/// PgBouncer-compatible tenant-scoped Postgres pool.
///
/// Wraps a shared [`PgPool`]. Isolation is enforced at the SQL level via fully
/// qualified table names — no `search_path` manipulation is ever performed.
pub struct TenantPool {
    pool: PgPool,
}

impl TenantPool {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Returns the underlying pool for control-schema queries.
    #[must_use]
    pub fn control(&self) -> &PgPool {
        &self.pool
    }

    /// Create the tenant schema for `ctx`.
    ///
    /// Uses `CREATE SCHEMA IF NOT EXISTS` so the call is idempotent.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Sqlx`] on database failure.
    pub async fn create_schema(&self, ctx: &TenantCtx) -> Result<(), StorageError> {
        let schema = ctx.schema_name();
        sqlx::query(&format!(r#"CREATE SCHEMA IF NOT EXISTS "{schema}""#))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Drop the tenant schema and all its contents.
    ///
    /// Uses `DROP SCHEMA IF EXISTS … CASCADE` so the call is idempotent.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Sqlx`] on database failure.
    pub async fn drop_schema(&self, ctx: &TenantCtx) -> Result<(), StorageError> {
        let schema = ctx.schema_name();
        sqlx::query(&format!(r#"DROP SCHEMA IF EXISTS "{schema}" CASCADE"#))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Returns `true` if the tenant schema exists in the database.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Sqlx`] on database failure.
    pub async fn schema_exists(&self, ctx: &TenantCtx) -> Result<bool, StorageError> {
        let schema = ctx.schema_name();
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(\
               SELECT 1 FROM information_schema.schemata \
               WHERE schema_name = $1\
             )",
        )
        .bind(schema)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists)
    }

    /// Delete all projection rows for `repo_id` from the tenant schema.
    ///
    /// Targets `code_files`, `code_symbols`, and `code_relations`. If the
    /// tenant schema does not exist the call is a no-op (idempotent).
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Sqlx`] on database failure.
    pub async fn delete_repo_data(
        &self,
        ctx: &TenantCtx,
        repo_id: uuid::Uuid,
    ) -> Result<(), StorageError> {
        if !self.schema_exists(ctx).await? {
            return Ok(());
        }
        let code_files = ctx.qualify("code_files");
        let code_symbols = ctx.qualify("code_symbols");
        let code_relations = ctx.qualify("code_relations");
        for table in [&code_files, &code_symbols, &code_relations] {
            sqlx::query(&format!("DELETE FROM {table} WHERE repo_id = $1"))
                .bind(repo_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
}
