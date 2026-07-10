//! Deliberately hanging build script (timeout-test fixture).
//!
//! `cargo check` on this crate compiles and runs this script, which sleeps
//! far longer than any test timeout — so a `cargo_check` tool call can only
//! finish via the wall-clock timeout killing cargo.
//!
//! The sleep is bounded (120s, not forever): killing cargo orphans the
//! build-script grandchild, and a bounded sleep guarantees the orphan exits
//! on its own shortly after the test instead of lingering for hours.
fn main() {
    std::thread::sleep(std::time::Duration::from_secs(120));
}
