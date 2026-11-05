//! UnrealPakBlock.cs: one compressed block, from its start to its end.

#[derive(Clone, Copy)]
pub struct Block {
    pub start: i64,
    pub end: i64,
}

impl Block {
    pub fn length(&self) -> i64 {
        self.end.wrapping_sub(self.start)
    }
}
