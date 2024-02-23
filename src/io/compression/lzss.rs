use std::io;
use std::io::Read;
use std::io::Write;

const N: usize = 0x1000;
const LZSS_NODE_NULL: usize = N;
const LZSS_FILL: u8 = 0x20;
const F: usize = 0x12;
const LZSS_MATCH_THRESHOLD: u8 = 0x2;
const LZSS_BUFFER_SIZE: usize = N + F - 1;

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
        let mut r: i32 = (N - F) as i32;
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

struct LzssMatch {
    pub position: usize,
    pub length:   usize
}

struct LzssContext {
    pub text_buffer:   [u8; LZSS_BUFFER_SIZE],
    pub code_buffer:   [u8; 17],
    pub left:          [usize; N + 1],
    pub right:         [usize; N + 257],
    pub parent:        [usize; N + 1],
    pub lzss_match:    LzssMatch
}

impl LzssContext {
    fn new() -> Self {
        Self {
            text_buffer: [LZSS_FILL; LZSS_BUFFER_SIZE],
            code_buffer: [0; 17],
            left: [LZSS_NODE_NULL; N + 1],
            right: [LZSS_NODE_NULL; N + 257],
            parent: [LZSS_NODE_NULL; N + 1],
            lzss_match: LzssMatch {
                position: 0,
                length: 0,
            },
        }
    }

    fn insertion_should_return(&mut self, node: usize, p: &mut usize, is_right: bool) -> bool {
        let side: &mut [usize] = match is_right {
            true => &mut self.right,
            false => &mut self.left
        };
        if side[*p] != LZSS_NODE_NULL {
            *p = side[*p];
            return false
        }
        side[*p] = node;
        self.parent[node] = *p;
        return true;
    }

    pub fn insert_node(&mut self, node: usize) {
        let mut cmp: u8 = 1; self.lzss_match.length = 0;
        let mut p = N + 1 + self.text_buffer[node] as usize;

        self.right[node] = LZSS_NODE_NULL;
        self.left[node] = LZSS_NODE_NULL;

        loop {
            match cmp {
                _ if { cmp != 0 }  => if self.insertion_should_return(node, &mut p, true) {
                    return;
                }
                _ => if self.insertion_should_return(node, &mut p, false) {
                    return;
                }
            }
            self.lzss_match.length = {
                let mut last_i = 0;
                for i in 1..F {
                    cmp = self.text_buffer[node + i] - self.text_buffer[p + i];
                    if cmp != 0 {
                        last_i = i;
                        break
                    }
                }
                if last_i <= self.lzss_match.length {
                    continue
                }
                last_i
            };
            self.lzss_match.position = p;
            if self.lzss_match.length >= F {
                break
            }
        }

        self.parent[node] = self.parent[p];
        self.left[node] = self.left[p];
        self.right[node] = self.right[p];
        self.parent[self.left[p]] = node;
        self.parent[self.right[p]] = node;
        if self.right[self.parent[p]] == p {
            self.right[self.parent[p]] = node;
            self.parent[p] = LZSS_NODE_NULL;
            return;
        }
        self.left[self.parent[p]] = node;
        self.parent[p] = LZSS_NODE_NULL

    }

    pub fn delete_node(&mut self, node: usize) {
        if self.parent[node] == LZSS_NODE_NULL {
            return;
        }

        let q = {
            if self.right[node] == LZSS_NODE_NULL {
                self.left[node]
            } else if self.left[node] == LZSS_NODE_NULL {
                self.right[node]
            } else {
                let mut temp_q: usize = self.left[node];
                if self.right[temp_q] != LZSS_NODE_NULL {
                    loop {
                        temp_q = self.right[temp_q];
                        if self.right[temp_q] == LZSS_NODE_NULL {
                            break;
                        }
                    }
                    self.right[self.parent[temp_q]] = self.left[temp_q];
                    self.parent[self.left[temp_q]] = self.parent[temp_q];
                    self.left[temp_q] = self.left[node];
                    self.parent[self.left[node]] = temp_q;
                }

                self.right[temp_q] = self.right[node];
                self.parent[self.right[node]] = temp_q;
                temp_q
            }
        };

        self.parent[q] = self.parent[node];
        if self.right[self.parent[node]] == node {
            self.right[self.parent[node]] = q;
            self.parent[node] = LZSS_NODE_NULL
        }
        self.left[self.parent[node]] = q;
        self.parent[node] = LZSS_NODE_NULL;
    }
}

pub trait LzssCompressionWriteExt: Write {

    // fn write_lzss(&mut self, data: &[u8]) -> io::Result<usize> {
    //     let mut len: usize = 0;
    //     let code_size: u32 = 0;
    //     let mask: u8 = 1;
    //     let code_index: u8 = 1;
    //     let s: usize = 0;
    //     let r: usize = N - F;
    //     let stop_pos = data.len();
    //     let mut input_index: usize = 0;
    //     let mut context = LzssContext::new();
    //
    //     while len < F && input_index < stop_pos {
    //         context.text_buffer[r + len] = data[input_index];
    //         input_index += 1; len += 1;
    //     }
    //
    //     for i in 1..=F {
    //         context.insert_node(r - i)
    //     }
    //
    //     Ok(len)
    // }
}