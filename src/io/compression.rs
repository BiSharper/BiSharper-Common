use std::io;
use std::io::{Read, Write};

const LZSS_WINDOW_SIZE: usize = 0x1000;
const LZSS_FILL: u8 = 0x20;
const LZSS_MATCH_MAX: usize = 0x12;
const LZSS_MATCH_THRESHOLD: u8 = 0x2;

fn lzss_decompression_helper(checksum: &mut i32, dst: &mut Vec<u8>, text_buf: &mut [u8], r: &mut i32, bytes_left: &mut usize, c: u8, signed_checksum: bool) {
    *checksum = if signed_checksum {
        checksum.wrapping_add(c as i8 as i32)
    } else {
        checksum.wrapping_add(c as i32)
    };

    dst.push(c);
    *bytes_left -= 1;
    text_buf[*r as usize] = c;
    *r = (*r + 1) & (LZSS_WINDOW_SIZE - 1) as i32;
}

pub trait CompressionReadExt: Read {
    fn read_u8(&mut self) -> io::Result<u8> {
        let mut buffer = [0u8; 1];
        self.read_exact(&mut buffer)?;
        Ok(buffer[0])
    }

    fn read_lzss(&mut self, expected_length: usize, signed_checksum: bool) -> io::Result<Vec<u8>> {
        let mut text_buf = [LZSS_FILL; LZSS_WINDOW_SIZE + LZSS_MATCH_MAX - 1];
        let mut bytes_left = expected_length;
        let mut dst = Vec::with_capacity(bytes_left);
        let mut i = 0;
        let mut j = 0;
        let mut r: i32 = (LZSS_WINDOW_SIZE - LZSS_MATCH_MAX) as i32;
        let mut c: u8= 0;
        let mut checksum: i32 = 0;
        let mut flags: i32 = 0;

        while bytes_left != 0 {

            flags >>= 1;

            if flags & 256 == 0 {
                c = Self::read_u8(self)?;
                flags = c as i32 | 0xff00;
            }

            if flags & 1 != 0 {
                c = Self::read_u8(self)?;
                lzss_decompression_helper(
                    &mut checksum, &mut dst, &mut text_buf,
                    &mut r, &mut bytes_left, c, signed_checksum,
                );
                flags >>= 1;
                continue
            }

            i = Self::read_u8(self)?; j = Self::read_u8(self)?;
            i |= (j & 0xf0) << 4; j &= 0x0f;
            j += LZSS_MATCH_THRESHOLD;
            let ii = r - i as i32;
            let jj = j as i32+ ii;
            if (j + 1) as usize> bytes_left {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "LZSS overflow",
                ));
            }
            for ii in ii..=jj {
                c = text_buf[(ii & (LZSS_WINDOW_SIZE - 1) as i32) as usize];
                lzss_decompression_helper(
                    &mut checksum, &mut dst, &mut text_buf,
                    &mut r, &mut bytes_left, c, signed_checksum,
                );
            }
        }
        let mut cs_data = [0u8; 4];
        self.read_exact(&mut cs_data)?;
        let csr = u32::from_le_bytes(cs_data);

        if csr != checksum as u32 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Checksum mismatch", ));
        }

        return Ok(dst)
    }

    fn read_cstring(&mut self) -> io::Result<String> {
        let mut bytes: Vec<u8> = Vec::new();
        for byte in self.bytes() {
            let b = byte?;
            if b == 0 {
                break;
            }
            bytes.push(b);
        }

        Ok(String::from_utf8(bytes).unwrap())
    }

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

pub trait CompressionWriteExt: Write {
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

    fn write_rv_string<S: AsRef<[u8]>>(&mut self, str: S) -> io::Result<()>{
        self.write_all(str.as_ref())?;
        self.write_all(b"\0")?;
        Ok(())
    }
}


impl<T: Write> CompressionWriteExt for T {

}
impl<T: Read> CompressionReadExt for T {

}