use std::io::Read;
use std::cmp::min;

pub struct Hole {
    cursor: usize // amount remaining
}

impl Hole {
    pub fn new(sz: usize) -> Self {
        Self {
            cursor: sz
        }
    }
}

impl Read for Hole {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.cursor == 0 {
            return Ok(0)
        }
        let l = min(buf.len(), self.cursor);
        for slot in &mut buf[0 .. l] {
            *slot = 0;
        }
        self.cursor -= l;
        Ok(l)
    }
}
