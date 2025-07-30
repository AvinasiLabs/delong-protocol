//! Background worker services for the secure service
//!
//! This module contains long-running background services that handle
//! blockchain synchronization, algorithm scheduling, and runtime execution.
//! These services typically run in separate tasks/threads and operate
//! continuously throughout the application lifecycle.

pub mod chainsync;
pub mod runtime;
pub mod scheduler;

// Re-export commonly used items
pub use scheduler::{AlgoScheduler, SchedulerConfig, SchedulerEvent, SchedulerHandler};
