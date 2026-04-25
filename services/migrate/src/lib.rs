#![allow(clippy::missing_errors_doc)]

mod error;
mod pg;

pub use error::MigrateError;
pub use pg::{
    control_status, migrate_all_tenants, migrate_control, migrate_tenant, tenant_schemas,
    tenant_status,
};
