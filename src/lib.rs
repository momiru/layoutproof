#![forbid(unsafe_code)]

pub mod cases;
pub mod ir;
pub mod lower;
pub mod oracle;
pub mod proof;
pub mod verify;

#[path = "../adapters/mod.rs"]
pub mod adapters;
