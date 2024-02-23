use std::io;
use std::io::{Read, Write};

pub trait IntCompressionReadExt: Read {
    fn read_bis_int(&mut self) -> io::Result<u32> {
        let mut result: u32 = 0;
        for (i, byte) in self.bytes().enumerate() {
            let b: u32 = byte?.into();
            result |= (b & 0x7f) << (i * 7);
            if b < 0x80 {
                break;
            }
        }
        Ok(result)
    }
}

impl<T: Read> IntCompressionReadExt for T {

}

pub const fn bis_int_len(x: u32) -> usize {
    let mut temp = x;
    let mut len = 0;

    while temp > 0x7f {
        len += 1;
        temp &= !0x7f;
        temp >>= 7;
    }

    len + 1
}

pub trait IntCompressionWriteExt: Write {
    fn write_bis_int(&mut self, x: u32) -> io::Result<usize> {
        let mut temp = x;
        let mut len = 0;

        while temp > 0x7f {
            self.write_all(&[(0x80 | temp & 0x7f) as u8])?;
            len += 1;
            temp &= !0x7f;
            temp >>= 7;
        }

        self.write_all(&[temp as u8])?;
        Ok(len + 1)
    }
}

impl<T: Write> IntCompressionWriteExt for T {

}