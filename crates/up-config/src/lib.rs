//! Strict configuration parsing shared by `up` and `up-dev` (ARCH-05, ARCH-11).
//!
//! No network and no filesystem writes: the crate only reads and validates.
pub mod schema;
pub mod yaml;
