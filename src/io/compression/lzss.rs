use std::fmt::Write;
use std::io;
use std::io::Read;

const LZSS_WINDOW_SIZE: usize = 0x1000;
const LZSS_FILL: u8 = 0x20;
const LZSS_MATCH_MAX: usize = 0x12;
const LZSS_MATCH_THRESHOLD: u8 = 0x2;
const LZSS_BUFFER_SIZE: usize = LZSS_WINDOW_SIZE + LZSS_MATCH_MAX - 1;

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

pub trait LzssCompressionReadExt: Read {
    fn read_u8(&mut self) -> io::Result<u8> {
        let mut buffer = [0u8; 1];
        self.read_exact(&mut buffer)?;
        Ok(buffer[0])
    }

    fn read_lzss(&mut self, expected_length: usize, signed_checksum: bool) -> io::Result<Vec<u8>> {
        let mut text_buf = [LZSS_FILL; LZSS_BUFFER_SIZE];
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
}

impl<T: Read> LzssCompressionReadExt for T {

}

struct LzssMatch {
    pub position: usize,
    pub length:   usize
}

struct LzssContext {
    text_buffer: [u8; LZSS_BUFFER_SIZE],
    left:        [usize; LZSS_WINDOW_SIZE + 1],
    right:       [usize; LZSS_WINDOW_SIZE + 257],
    parent:      [usize; LZSS_WINDOW_SIZE + 1],
    last_match:  LzssMatch
}

impl LzssContext {
    fn new() -> Self {
        let mut right: [usize; LZSS_WINDOW_SIZE + 257] = [0; LZSS_WINDOW_SIZE + 257];
        for i in (LZSS_WINDOW_SIZE + 1..=LZSS_WINDOW_SIZE + 256).into_iter() {
            right[i] = LZSS_WINDOW_SIZE;
        }
        Self {
            text_buffer: [LZSS_FILL; LZSS_BUFFER_SIZE],
            left: [0; LZSS_WINDOW_SIZE + 1],
            right,
            parent: [LZSS_WINDOW_SIZE; LZSS_WINDOW_SIZE + 1],
            last_match: LzssMatch {
                position: 0,
                length: 0
            },
        }

    }
}

pub trait LzssCompressionWriteExt: Write {

    // fn write_lzss(&mut self, data: &[u8], maximum_size: usize) -> io::Result<Vec<u8>> {
    //     let code_size: u32 = 0;
    //     let mask: u8 = 1;
    //     let code_index: u8 = 1;
    //     let s: usize = 0;
    //     let r: usize = LZSS_WINDOW_SIZE - LZSS_MATCH_MAX;
    //     let mut input_index: usize = 0;
    //     let mut code_buffer: [u8; 17];
    //
    //     todo!()
    // }
}