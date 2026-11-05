//! The ABI v2 listing: UTF-8 JSON Lines, the container first, then one line per entry.

use crate::host::{Container, Refused, TOO_LARGE};
use crate::pak::in_place::Range;
use crate::pak::{listing, locate, Failed};
use std::fmt::Write;

pub fn list_pak(file: &Container) -> Result<Vec<u8>, Refused> {
    let pak = match listing::read(file) {
        Ok(pak) => pak,
        Err(Failed::Problem(problem)) => return Err(Refused(listing::refusal(problem))),
        Err(Failed::Host(refused)) => return Err(refused),
    };

    let mut lines = Lines::default();
    let proven = locate::proven(&pak);
    let mut line = String::from("{\"archive\":{");
    let _ = write!(line, "\"indexAt\":{},\"indexLength\":{},\"mount\":", proven, file.len().wrapping_sub(proven));
    quote(&mut line, &pak.index.mount_point);
    line.push_str("}}");
    lines.add(&line)?;

    let located = locate::every(file, &pak)?;
    for (name, one) in pak.names.iter().zip(located) {
        line.clear();
        line.push_str("{\"path\":");
        quote(&mut line, &name.path);
        match one {
            Ok(range) => entry(&mut line, &range),
            Err(problem) => {
                line.push_str(",\"problem\":");
                quote(&mut line, problem);
            }
        }
        line.push('}');
        lines.add(&line)?;
    }

    Ok(lines.out)
}

fn entry(line: &mut String, r: &Range) {
    let _ = write!(
        line,
        ",\"offset\":{},\"length\":{},\"method\":{},\"size\":{},\"decoder\":\"{}\",\"blockSize\":{},\"blocks\":[",
        r.offset, r.length, r.method, r.size, r.decoder, r.block_size
    );
    for (i, b) in r.blocks.iter().enumerate() {
        if i > 0 {
            line.push(',');
        }
        let _ = write!(line, "{}", b);
    }
    line.push(']');
}

fn quote(line: &mut String, text: &str) {
    line.push('"');
    for c in text.chars() {
        match c {
            '"' => line.push_str("\\\""),
            '\\' => line.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(line, "\\u{:04x}", c as u32);
            }
            c => line.push(c),
        }
    }
    line.push('"');
}

#[derive(Default)]
struct Lines {
    out: Vec<u8>,
}

impl Lines {
    fn add(&mut self, line: &str) -> Result<(), Refused> {
        self.out.try_reserve(line.len() + 1).map_err(|_| Refused(TOO_LARGE))?;
        self.out.extend_from_slice(line.as_bytes());
        self.out.push(b'\n');
        Ok(())
    }
}
