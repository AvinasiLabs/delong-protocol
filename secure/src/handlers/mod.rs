pub mod health;
pub mod dataset;
pub mod algo_exe;
pub mod runtime;
pub mod committee;
pub mod votes;
pub mod contracts;
pub mod reports;

// Re-export commonly used handler functions
pub use health::health_check;
pub use dataset::{list_datasets, get_dataset};
pub use algo_exe::{submit_algorithm_execution, list_executions, get_execution}; 