//! UnrealPakEncoded.cs: one entry as the primary index encodes it.

#[allow(dead_code)]
pub struct Encoded {
    pub offset: i64,
    pub size: i64,
    pub stored: i64,
    pub method: u32,
    pub encrypted: bool,
    pub blocks: i32,
    pub block_size: i64,
}
