use std::io;
use std::io::{Read, Write};

pub trait CommonReadExt: Read {
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

}

impl<T: Read> CommonReadExt for T {

}

pub trait CommonWriteExt: Write {
    fn write_rv_string<S: AsRef<[u8]>>(&mut self, str: S) -> io::Result<()>{
        self.write_all(str.as_ref())?;
        self.write_all(b"\0")?;
        Ok(())
    }
}

impl<T: Write> CommonWriteExt for T {

}