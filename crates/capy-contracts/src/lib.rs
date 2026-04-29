//! Shared Capybara wire contracts.
//!
//! This crate owns process-boundary types that must stay stable across the CLI,
//! shell, frontend bridge, and verification harness.

pub mod canvas;
pub mod ipc;
pub mod timeline;
