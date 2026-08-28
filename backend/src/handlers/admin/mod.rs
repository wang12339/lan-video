pub mod admin_logs;
pub mod admin_performance;
pub mod admin_plan;
pub mod admin_system;
pub mod admin_tenant;
pub mod admin_transcode;
pub mod admin_user;
pub mod admin_video;

pub use admin_logs::*;
pub use admin_performance::*;
pub use admin_plan::*;
pub use admin_system::*;
pub use admin_tenant::*;
pub use admin_transcode::*;
pub use admin_user::*;
pub use admin_video::*;

use axum::http::StatusCode;
use axum::Json;

use crate::util::error::ServiceError;
use crate::util::response::ErrorResponse;

pub fn map_admin_err(e: ServiceError) -> (StatusCode, Json<ErrorResponse>) {
    e.into_tuple()
}
