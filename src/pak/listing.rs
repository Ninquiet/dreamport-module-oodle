//! UnrealPakListing.cs: a pak's footer, primary index and names, without the mount strip (the host strips).

use super::directory;
use super::footer::{self, Footer};
use super::index::{self, Index};
use super::name::Name;
use super::unpack;
use super::Failed;
use crate::host::Container;

pub struct Listing {
    pub footer: Footer,
    pub index: Index,
    pub names: Vec<Name>,
}

pub fn read(file: &Container) -> Result<Listing, Failed> {
    let footer = footer::read(file, file.len())?;
    if footer.encrypted {
        return Err(unpack::ENCRYPTED.into());
    }

    let index_bytes = file.at(footer.index_offset, footer.index_size as i32 as i64)?.ok_or(index::SHORT)?;
    let index = index::read(&index_bytes, file.len(), footer.version)?;
    drop(index_bytes);

    let dir_bytes = file.at(index.full_directory_index_offset, index.full_directory_index_size as i32 as i64)?.ok_or(index::SHORT)?;
    let names = directory::read(&dir_bytes)?;

    Ok(Listing { footer, index, names })
}

/// What `dp_list` returns for a pak it cannot list: -2 for what this reader does not open, -1 for the rest.
pub fn refusal(problem: &str) -> i64 {
    const UNSUPPORTED: [&str; 4] = [unpack::ENCRYPTED, index::OLDER_PAK, index::NO_NAMES, index::OLD_ENTRIES];
    if UNSUPPORTED.contains(&problem) {
        -2
    } else {
        -1
    }
}
