//! The one import a reader may use (ABI v2): `dp_host.read`, which reads the container the host has open
//! during `dp_list`, and nothing else. A module that never calls it imports nothing.

#[link(wasm_import_module = "dp_host")]
extern "C" {
    #[link_name = "read"]
    fn dp_host_read(offset: i64, dst_ptr: *mut u8, len: i32) -> i32;
}

pub const MOST_PER_READ: usize = 16 << 20;

pub const CORRUPT: i64 = -1;
pub const TOO_LARGE: i64 = -3;
pub const BAD_ARGUMENTS: i64 = -5;

/// Why a read gave nothing, as the negative result `dp_list` returns for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused(pub i64);

/// The container being listed, as long as `dp_list` was told.
#[derive(Clone, Copy)]
pub struct Container {
    len: i64,
}

impl Container {
    pub fn new(len: i64) -> Container {
        Container { len }
    }

    pub fn len(&self) -> i64 {
        self.len
    }

    /// Fills `into` from `offset`, in reads of at most 16 MiB. A host refusal is `BAD_ARGUMENTS`; fewer bytes than asked is `CORRUPT`.
    pub fn read(&self, offset: i64, into: &mut [u8]) -> Result<(), Refused> {
        let mut done = 0usize;
        while done < into.len() {
            let piece = (into.len() - done).min(MOST_PER_READ);
            let at = offset.checked_add(done as i64).ok_or(Refused(BAD_ARGUMENTS))?;
            let dst = into[done..].as_mut_ptr();
            let got = unsafe { dp_host_read(at, dst, piece as i32) };
            if got < 0 {
                return Err(Refused(BAD_ARGUMENTS));
            }
            if got as usize != piece {
                return Err(Refused(CORRUPT));
            }
            done += piece;
        }
        Ok(())
    }

    /// `length` bytes from `offset`, or `None` when they do not lie inside the container; the host is not asked then.
    pub fn at(&self, offset: i64, length: i64) -> Result<Option<Vec<u8>>, Refused> {
        if offset < 0 || length < 0 || offset > self.len - length {
            return Ok(None);
        }
        let size = usize::try_from(length).map_err(|_| Refused(TOO_LARGE))?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(size).map_err(|_| Refused(TOO_LARGE))?;
        bytes.resize(size, 0);
        self.read(offset, &mut bytes)?;
        Ok(Some(bytes))
    }
}
