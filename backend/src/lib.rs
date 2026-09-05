#![recursion_limit = "512"]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod app;
pub mod config;
pub mod db;
pub mod handlers;
pub mod metrics;
pub mod middleware;
pub mod models;
pub mod openapi;
pub mod repositories;
pub mod services;
pub mod state;
pub mod util;
