// File-system subsystem.
//
// Sections are kept in a single submodule file `code.rs` to keep the
// large-diff refactor manageable. See MODULE_LAYOUT.md for the logical
// breakdown by struct/concern.

pub mod code;
pub use code::*;
