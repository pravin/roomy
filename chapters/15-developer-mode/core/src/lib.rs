//! Everything Roomy knows about disks, with no user interface attached.
//! The command line tool and the desktop app both sit on top of this crate.

pub mod junk;
pub mod kinds;
pub mod scan;
pub mod size;
pub mod tree;
pub mod treemap;

pub use scan::{Progress, ScanError, ScanOptions, scan};
pub use size::format_size;
pub use tree::{Kind, Node, NodeId, Tree};
