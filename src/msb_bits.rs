//! MsbBits.cs: a bit stream whose first bit read is the most significant of each value, forward or backward over a region; past the region it reads zeros.

#[derive(Clone, Copy)]
pub struct MsbBits<'a> {
    data: &'a [u8],
    start: i32,
    end: i32,
    backward: bool,
    next: i32,
    window: u64,
    held: i32,
    consumed: i64,
}

impl<'a> MsbBits<'a> {
    pub fn new(data: &'a [u8], start: i32, end: i32, backward: bool) -> Self {
        MsbBits { data, start, end, backward, next: if backward { end } else { start }, window: 0, held: 0, consumed: 0 }
    }

    pub fn region_length(&self) -> i32 {
        self.end - self.start
    }

    pub fn bytes_touched(&self) -> i64 {
        (self.consumed + 7) >> 3
    }

    pub fn overran(&self) -> bool {
        self.bytes_touched() > self.region_length() as i64
    }

    #[inline(always)]
    pub fn peek(&mut self, count: i32) -> u32 {
        if self.held < count {
            self.fill();
        }
        ((self.window.wrapping_shr((self.held - count) as u32)) & (1u64.wrapping_shl(count as u32).wrapping_sub(1))) as u32
    }

    #[inline(always)]
    pub fn skip(&mut self, count: i32) {
        self.held -= count;
        self.consumed += count as i64;
    }

    #[inline(always)]
    pub fn read(&mut self, count: i32) -> u32 {
        if count == 0 {
            return 0;
        }
        let value = self.peek(count);
        self.skip(count);
        value
    }

    pub fn read_long(&mut self, count: i32) -> i64 {
        if count <= 24 {
            return self.read(count) as i64;
        }
        let high = self.read(count - 24) as i64;
        (high << 24) | self.read(24) as i64
    }

    #[inline(always)]
    pub fn leading_zeros(&mut self) -> i32 {
        self.peek(32).leading_zeros() as i32
    }

    fn fill(&mut self) {
        let bytes = (64 - self.held) >> 3;
        let room = if self.backward { self.next - self.start >= 8 } else { self.end - self.next >= 8 };
        if room {
            let next = if self.backward {
                let at = (self.next - 8) as usize;
                u64::from_le_bytes(self.data[at..at + 8].try_into().unwrap())
            } else {
                let at = self.next as usize;
                u64::from_be_bytes(self.data[at..at + 8].try_into().unwrap())
            };
            self.window = if bytes == 8 {
                next
            } else {
                self.window.wrapping_shl((8 * bytes) as u32) | next.wrapping_shr((64 - 8 * bytes) as u32)
            };
            self.next += if self.backward { -bytes } else { bytes };
            self.held += 8 * bytes;
            return;
        }
        while self.held <= 56 {
            let mut value = 0u32;
            if self.backward {
                if self.next > self.start {
                    self.next -= 1;
                    value = self.data[self.next as usize] as u32;
                }
            } else if self.next < self.end {
                value = self.data[self.next as usize] as u32;
                self.next += 1;
            }
            self.window = self.window.wrapping_shl(8) | value as u64;
            self.held += 8;
        }
    }
}
