pub mod blockchain_sync;
pub mod event_handler;

// Re-export main types
pub use blockchain_sync::BlockchainSyncService;
pub use event_handler::EventHandler; 