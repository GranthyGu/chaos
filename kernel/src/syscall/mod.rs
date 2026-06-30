// Syscall layer.
//
// The actual `dispatch_syscall` method lives on the `Kernel` aggregate in
// `kernel_core.rs` for now (because it needs `&self` access to every
// subsystem). A future cleanup step (Task 2) can extract it onto a
// `Syscalls` trait implemented in this module.

// Intentionally empty for now.
