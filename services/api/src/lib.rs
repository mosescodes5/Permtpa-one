//! PERMTPA ONE API library (kept separate from `main` so it is testable end to end).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
#![allow(clippy::collapsible_if, clippy::type_complexity)]
pub mod app;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod inventory;
pub mod mailer;
pub mod mfa;
pub mod onboarding;
pub mod orgs;
pub mod ratelimit;
pub mod security;
pub mod session;
pub mod team;
pub mod validate;
