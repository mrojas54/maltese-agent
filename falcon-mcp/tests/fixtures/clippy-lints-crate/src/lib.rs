//! Fixture for cargo_clippy's `warn` passthrough (MA-34).
//!
//! `clippy::unwrap_used` is an allow-by-default *restriction* lint: a plain
//! `cargo clippy` never emits it, so `cargo_clippy` returns `lints: []` here.
//! Passing `warn: ["clippy::unwrap_used"]` must make it fire — that is the
//! whole behavior under test. The crate otherwise compiles with zero default
//! warnings so the without-`warn` case is unambiguously empty.

/// Returns the inner value, panicking on `None`. The `.unwrap()` is the
/// deliberate `clippy::unwrap_used` trigger; nothing here warns by default.
pub fn unwrap_it(x: Option<i32>) -> i32 {
    x.unwrap()
}
