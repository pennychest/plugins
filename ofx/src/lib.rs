//! PennyChest's importer for OFX files. `main.rs` serves it as a plugin.

pub mod importer;
pub mod reader;

pub use importer::Ofx;
