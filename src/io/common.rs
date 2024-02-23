use std::io::Read;

pub trait CommonReadExt: Read {

}

impl<T: Read> CommonReadExt for T {

}