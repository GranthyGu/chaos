// File-system subsystem.
//
// Sections are kept in a single submodule file `code.rs` to keep the
// large-diff refactor manageable. See MODULE_LAYOUT.md for the logical
// breakdown by struct/concern.
//
// The `audit` submodule is separate: it is a new feature layered on top
// of the fs primitives (see fs/audit.rs) rather than being extracted
// from the original monolithic kernel.rs.

pub mod code;
pub mod audit;

pub use code::*;
pub use audit::*;
