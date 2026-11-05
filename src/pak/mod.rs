//! The reader `unrealpak`: every entry of one Unreal .pak and where its bytes lie, nothing decoded.
//! A translation of Somnium's C# reader (`Unpackers/Pak/*.cs`), one Rust file per C# file; selection stays in the host.

pub mod block;
pub mod directory;
pub mod encoded;
pub mod entry;
pub mod footer;
pub mod in_place;
pub mod index;
pub mod le;
pub mod listing;
pub mod locate;
pub mod name;
pub mod record;
pub mod record_head;
pub mod unpack;

use crate::host::Refused;

/// A C# problem string, or a read the host refused, which ends the whole listing.
pub enum Failed {
    Problem(&'static str),
    Host(Refused),
}

impl From<&'static str> for Failed {
    fn from(problem: &'static str) -> Failed {
        Failed::Problem(problem)
    }
}

impl From<Refused> for Failed {
    fn from(refused: Refused) -> Failed {
        Failed::Host(refused)
    }
}
