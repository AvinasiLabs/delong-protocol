pub mod algo_exe;
pub mod committee;
pub mod contract;
pub mod dataset;
pub mod health;
pub mod not_found;
pub mod vote;
pub mod websocket;

// Re-export handler functions for convenience
pub use algo_exe::{get_algo_exe, list_algo_exes, submit_algo_exe};
pub use committee::{
    get_committee_member, is_committee_member, list_committee_members, set_committee_member,
};
pub use contract::list_contracts;
pub use dataset::{create_dataset, list_datasets};
pub use health::health_check;
pub use not_found::handler as not_found_handler;
pub use vote::{list_votes, set_voting_duration};
pub use websocket::{websocket_handler, ws_routes, WsState};
