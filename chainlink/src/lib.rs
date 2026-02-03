//! Chainlink issue tracker library
//!
//! This module exposes the core functionality for use in fuzzing and testing.
//!
//! # Features
//!
//! - `std` (default): Enable standard library support
//! - `rusqlite-backend` (default): Enable the rusqlite database backend
//!
//! # no_std Support
//!
//! This crate supports `no_std` environments when compiled without the `std` feature.
//! In this mode, you must provide your own `DatabaseBackend` implementation and
//! implement the `Output` trait for custom output handling.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod backend;
pub mod commands;
pub mod db;
pub mod models;
pub mod output;
pub mod utils;
