use crate::codec::{checksum, Frame, FrameError, FrameType, END, ESCAPE, ESCAPE_MASK, START};

/// Byte-at-a-time frame decoder. Bytes outside a frame are dropped.
#[derive(Debug, Default)]
pub struct Decoder {
    in_frame: bool,
    escape_next: bool,
    inner: Vec<u8>,
}

impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, bytes: &[u8]) -> Vec<Result<Frame, FrameError>> {
        let mut out = Vec::new();
        for &b in bytes {
            if !self.in_frame {
                if b == START {
                    self.in_frame = true;
                }
                continue;
            }
            match b {
                START => {
                    // A new frame began before the previous one ended.
                    out.push(Err(FrameError::Truncated));
                    self.reset();
                    self.in_frame = true;
                }
                END => {
                    out.push(parse(&self.inner));
                    self.reset();
                }
                ESCAPE => self.escape_next = true,
                _ if self.escape_next => {
                    self.escape_next = false;
                    // Unescaped byte: the transmitted value had bit 4 cleared.
                    self.inner.push(b | !ESCAPE_MASK);
                }
                _ => self.inner.push(b),
            }
        }
        out
    }

    fn reset(&mut self) {
        self.in_frame = false;
        self.escape_next = false;
        self.inner.clear();
    }
}

fn parse(inner: &[u8]) -> Result<Frame, FrameError> {
    if inner.len() < 7 {
        return Err(FrameError::BadLength);
    }
    let len = u32::from_be_bytes([inner[2], inner[3], inner[4], inner[5]]) as usize;
    if len != inner.len() - 7 {
        return Err(FrameError::BadLength);
    }
    let (body, sum) = inner.split_at(inner.len() - 1);
    let expected = checksum(body);
    if expected != sum[0] {
        return Err(FrameError::BadChecksum { expected, got: sum[0] });
    }
    Ok(Frame { frame_type: FrameType::from_u8(inner[0]), seq: inner[1], payload: inner[6..inner.len() - 1].to_vec() })
}
