//! UnrealPakUnpack.cs: the two problems a listing shares with the reader.

pub const ENCRYPTED: &str = "this pak is encrypted, which this reader does not open";
pub const LARGER: &str = "an entry's record declares more bytes than the pak's index gave it";
