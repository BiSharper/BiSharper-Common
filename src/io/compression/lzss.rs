use std::io;
use std::io::Read;
use std::io::Write;
use byteorder::ReadBytesExt;

const N: u32 = 0x1000;
const LZSS_NODE_NULL: u32 = N;
const LZSS_FILL: u8 = 0x20;
const F: u32 = 0x12;
const LZSS_MATCH_THRESHOLD: u8 = 0x2;
const LZSS_BUFFER_SIZE: u32 = N + F - 1;

fn lzss_decompression_helper(checksum: &mut i32, dst: &mut Vec<u8>, text_buf: &mut [u8], r: &mut i32, bytes_left: &mut usize, c: u8, signed_checksum: bool) {
    *checksum = if signed_checksum {
        checksum.wrapping_add(c as i8 as i32)
    } else {
        checksum.wrapping_add(c as i32)
    };

    dst.push(c);
    *bytes_left -= 1;
    text_buf[*r as usize] = c;
    *r = (*r + 1) & (N - 1) as i32;
}

pub trait LzssCompressionReadExt: Read {
    fn read_lzss(&mut self, expected_length: usize, signed_checksum: bool) -> io::Result<Vec<u8>> {
        let mut text_buf = [LZSS_FILL; LZSS_BUFFER_SIZE as usize];
        let mut bytes_left = expected_length;
        let mut dst = Vec::with_capacity(bytes_left);
        let mut r: i32 = (N - F) as i32;
        let mut checksum: i32 = 0;
        let mut flags: i32 = 0;

        while bytes_left != 0 {
            let mut c: u8;

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
            let mut i = self.read_u8()?;
            let mut j = self.read_u8()?;
            i |= (j & 0xf0) << 4; j &= 0x0f;
            j += LZSS_MATCH_THRESHOLD;
            let ii = r - i as i32;
            let jj = j as i32 + ii;
            if (j + 1) as usize> bytes_left {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "LZSS overflow",
                ));
            }
            for ii in ii..=jj {
                c = text_buf[(ii & (N - 1) as i32) as usize];
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

struct LzssContext {
    pub text_buffer:    [u8; LZSS_BUFFER_SIZE as usize],
    pub left:           [u32; (N + 1) as usize],
    pub right:          [u32; (N + 257) as usize],
    pub parent:         [u32; (N + 1) as usize],
    pub match_position: u32,
    pub match_length:   u32
}

impl LzssContext {
    fn new() -> Self {
        Self {
            text_buffer: [LZSS_FILL; LZSS_BUFFER_SIZE as usize],
            left: [LZSS_NODE_NULL; (N + 1) as usize],
            right: [LZSS_NODE_NULL; (N + 257) as usize],
            parent: [LZSS_NODE_NULL; (N + 1) as usize],
            match_position: 0,
            match_length: 0
        }
    }

    fn insertion_should_return(&mut self, node: u32, p: &mut u32, is_right: bool) -> bool {
        let side: &mut [u32] = match is_right {
            true => &mut self.right,
            false => &mut self.left
        };
        if side[*p as usize] != LZSS_NODE_NULL {
            *p = side[*p as usize];
            return false
        }
        side[*p as usize] = node;
        self.parent[node as usize] = *p;
        return true;
    }

    pub fn insert_node(&mut self, node: u32) {
        let mut cmp: u8 = 1; self.match_length = 0;
        let mut p = N + 1 + self.text_buffer[node as usize] as u32;

        self.right[node as usize] = LZSS_NODE_NULL;
        self.left[node as usize] = LZSS_NODE_NULL;

        loop {
            match cmp {
                _ if { cmp != 0 }  => if self.insertion_should_return(node, &mut p, true) {
                    return;
                }
                _ => if self.insertion_should_return(node, &mut p, false) {
                    return;
                }
            }
            self.match_length = {
                let mut last_i = 0;
                for i in 1..F {
                    cmp = self.text_buffer[(node + i) as usize] - self.text_buffer[(p + i) as usize];
                    if cmp != 0 {
                        last_i = i;
                        break
                    }
                }
                if last_i <= self.match_length {
                    continue
                }
                last_i
            };
            self.match_position = p;
            if self.match_length >= F {
                break
            }
        }

        self.parent[node as usize] = self.parent[p as usize];
        self.left[node as usize] = self.left[p as usize];
        self.right[node as usize] = self.right[p as usize];
        self.parent[self.left[p as usize] as usize] = node;
        self.parent[self.right[p as usize] as usize] = node;
        if self.right[self.parent[p as usize] as usize] == p {
            self.right[self.parent[p as usize] as usize] = node;
            self.parent[p as usize] = LZSS_NODE_NULL;
            return;
        }
        self.left[self.parent[p as usize] as usize] = node;
        self.parent[p as usize] = LZSS_NODE_NULL
    }

    pub fn delete_node(&mut self, node: u32) {
        if self.parent[node as usize] == LZSS_NODE_NULL {
            return;
        }

        let q = {
            if self.right[node as usize] == LZSS_NODE_NULL {
                self.left[node as usize]
            } else if self.left[node as usize] == LZSS_NODE_NULL {
                self.right[node as usize]
            } else {
                let mut temp_q: u32 = self.left[node as usize];
                if self.right[temp_q as usize] != LZSS_NODE_NULL {
                    loop {
                        temp_q = self.right[temp_q as usize];
                        if self.right[temp_q as usize] == LZSS_NODE_NULL {
                            break;
                        }
                    }
                    self.right[self.parent[temp_q as usize] as usize] = self.left[temp_q as usize];
                    self.parent[self.left[temp_q as usize] as usize] = self.parent[temp_q as usize];
                    self.left[temp_q as usize] = self.left[node as usize];
                    self.parent[self.left[node as usize] as usize] = temp_q;
                }

                self.right[temp_q as usize] = self.right[node as usize];
                self.parent[self.right[node as usize] as usize] = temp_q;
                temp_q
            }
        };

        self.parent[q as usize] = self.parent[node as usize];
        if self.right[self.parent[node as usize] as usize] == node {
            self.right[self.parent[node as usize] as usize] = q;
            self.parent[node as usize] = LZSS_NODE_NULL
        }
        self.left[self.parent[node as usize] as usize] = q;
        self.parent[node as usize] = LZSS_NODE_NULL;
    }
}

pub trait LzssCompressionWriteExt: Write {

    fn write_lzss(&mut self, data: &[u8]) -> io::Result<u32> {
        let mut len: u32 = 0;
        let mut code_size: u32 = 0;
        let mut code_buffer: [u8; 17] = [0; 17];
        let mut mask: u8 = 1;
        let mut code_index: u8 = 1;
        let mut s: u32 = 0;
        let mut r: u32 = N - F;
        let stop_pos = data.len();
        let mut data_index: usize = 0;
        let mut context = LzssContext::new();

        while len < F && data_index < stop_pos {
            context.text_buffer[(r + len) as usize] = data[data_index];
            data_index += 1; len += 1;
        }

        for i in 1..=F { context.insert_node(r - i) }
        context.insert_node(r);

        loop {
            if context.match_length > len {
                context.match_length = len
            }

            match context.match_length <= LZSS_MATCH_THRESHOLD as u32 {
                true => {
                    context.match_length = 1;
                    code_buffer[0] |= mask;
                    code_buffer[code_index as usize] = context.text_buffer[r as usize];
                    code_index += 1;
                }
                false => {
                    let encoded_position = (r - context.match_position) & (N - 1);
                    code_buffer[code_index as usize] = encoded_position as u8;
                    code_index += 1;
                    code_buffer[code_index as usize] = (((encoded_position >> 4) & 0xf0) | (context.match_length - (LZSS_MATCH_THRESHOLD + 1) as u32)) as u8;
                    code_index += 1;
                }
            }

            mask <<= 1;
            if mask == 0 {
                self.write(&code_buffer[..code_index as usize])?;
                code_size += code_index as u32;
                code_buffer[0] = 0;
                mask = 1;
                code_index = 1;
            }

            {
                let previous_match_length = context.match_length;
                let mut i = 0;
                while i < previous_match_length && data_index < stop_pos {
                    context.delete_node(s);
                    let c = data[data_index];
                    data_index += 1;
                    context.text_buffer[s as usize] = c;
                    if s < F - 1 {
                       context.text_buffer[(s + N) as usize] = c;
                    }

                    s = (s + 1) & (N - 1);
                    r = (r + 1) & (N - 1);
                    context.insert_node(r);
                    i += 1;
                }

                while i < previous_match_length {
                    context.delete_node(s);
                    s = (s + 1) & (N - 1);
                    r = (r + 1) & (N - 1);
                    len -= 1;
                    if len != 0 {
                       context.insert_node(r)
                    }
                }
            }
            if len == 0 {
               break
            }
        }

        if code_index > 1 {
            self.write(&code_buffer[..code_index as usize])?;
            code_size += code_index as u32
        }

        Ok(code_size)
    }
}