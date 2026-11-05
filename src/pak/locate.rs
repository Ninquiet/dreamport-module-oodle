//! PakLocate.cs: every entry located, reading the records in the order they lie, nothing decoded.
//! The pak's proof is its own index to its end.

use super::directory;
use super::encoded::Encoded;
use super::in_place::{self, Range};
use super::listing::Listing;
use super::unpack;
use super::Failed;
use crate::host::{Container, Refused};

/// Each name's entry, in the pak's own order, or the problem the C# reader refuses it with when it is selected.
pub fn every(file: &Container, listing: &Listing) -> Result<Vec<Result<Range, &'static str>>, Refused> {
    let encoded: Vec<Result<Encoded, &'static str>> = listing.names.iter().map(|one| choose(&listing.index.encoded, one.at)).collect();
    let mut found: Vec<Result<Range, &'static str>> = encoded.iter().map(|e| e.as_ref().map(|_| Range::default()).map_err(|p| *p)).collect();

    let mut order: Vec<usize> = (0..encoded.len()).filter(|&i| encoded[i].is_ok()).collect();
    order.sort_by_key(|&i| encoded[i].as_ref().map_or(0, |e| e.offset));

    for i in order {
        let Ok(e) = &encoded[i] else { continue };
        found[i] = match in_place::locate(file, file.len(), &listing.footer, e) {
            Ok(range) => Ok(range),
            Err(Failed::Problem(problem)) => Err(problem),
            Err(Failed::Host(refused)) => return Err(refused),
        };
    }

    Ok(found)
}

/// PakEntryChoice's checks on a selected entry, before it is located.
fn choose(blob: &[u8], at: i32) -> Result<Encoded, &'static str> {
    let encoded = directory::entry(blob, at)?;
    if encoded.encrypted {
        return Err(unpack::ENCRYPTED);
    }
    Ok(encoded)
}

pub fn proven(listing: &Listing) -> i64 {
    let at = listing.footer.index_offset.min(listing.index.full_directory_index_offset);
    if listing.index.has_path_hash_index {
        at.min(listing.index.path_hash_index_offset)
    } else {
        at
    }
}
