// Trap and interrupt handling.
//
//   * context - Context struct (CPU register snapshot)
//   * ctrl    - TrapCtl: interrupt masks, frame stack, dispatch_vector

pub mod context;
pub mod ctrl;

pub use context::*;
pub use ctrl::*;
