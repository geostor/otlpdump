//! Shared code for the otlptools: reading otlpdump output, walking the records in it,
//! and filtering them.

pub mod attrs;
pub mod filter;
pub mod format;
pub mod input;
pub mod record;

pub use filter::{Filter, FilterArgs};
pub use record::{Batch, Item, Kind, Record};
