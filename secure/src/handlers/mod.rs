pub mod health;
pub mod dataset;
pub mod algo_exe;
pub mod runtime;

// Re-export commonly used handler functions
pub use health::health_check;
pub use dataset::{list_datasets, get_dataset};
pub use algo_exe::{list_executions, get_execution}; 