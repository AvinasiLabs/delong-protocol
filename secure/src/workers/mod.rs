//! Background worker services for the secure service
//!
//! This module contains long-running background services that handle
//! blockchain synchronization and algorithm execution.
//! These services typically run in separate tasks/threads and operate
//! continuously throughout the application lifecycle.

pub mod chainsync;
pub mod executor;

// Re-export commonly used items
pub use crate::config::ExecutorConfig;
pub use chainsync::transaction_monitor::{TransactionMonitor, TransactionMonitorConfig};
pub use chainsync::ChainSyncWorker;
pub use executor::{Executor, ExecutorError};
