//! PRISM Core Engine
//! The orchestrator and heart of the Proactive Repair and Intelligent System Management application.

pub mod client;

// Re-export the client and the generated protobuf models for use by other crates
pub use client::{prism, PrismClient};
