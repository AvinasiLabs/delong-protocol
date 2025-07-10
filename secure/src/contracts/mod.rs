//! This module contains the auto-generated contract bindings from ABIs.
use ethers::prelude::abigen;

// Generate bindings for the DataContribution contract
abigen!(
    DataContribution,
    "src/abi/data_contribution.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

// Generate bindings for the AlgorithmReview contract
abigen!(
    AlgorithmReview,
    "src/abi/algorithm_review.json",
    event_derives(serde::Deserialize, serde::Serialize)
); 