//! A kernel-neutral conformance suite for B-rep operations: generated cases with what is known
//! about them, the contract's checks, and a runner that drives any kernel through
//! [`brep_protocol::Kernel`], in process or as a worker speaking the protocol.

pub use brep_protocol::{model, profile};

pub mod assets;
pub mod cli;
mod display;
pub mod generate;
pub mod glb;
pub mod html;
pub mod mass;
pub mod oracle;
pub use brep_protocol::portable;
pub mod registry;
pub mod report;
pub mod run;
pub mod sound;
pub mod step;
pub mod views;
