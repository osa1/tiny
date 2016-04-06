use std::io::Write;
use std::io;
use std::str;

pub fn find_byte(buf : &[u8], byte0 : u8) -> Option<usize> {
    for (byte_idx, byte) in buf.iter().enumerate() {
        if *byte == byte0 {
            return Some(byte_idx);
        }
    }
    None
}

pub fn log_stderr_bytes(msg : &str, bytes : &[u8]) {
    match str::from_utf8(bytes) {
        Err(_) => {
            writeln!(io::stderr(), "{} {:?}", msg, bytes).unwrap();
        },
        Ok(ascii) => {
            writeln!(io::stderr(), "{} {}", msg, ascii).unwrap();
        }
    }
}

pub fn drop_port(s : &str) -> Option<&str> {
    s.find(':').map(|split| &s[ 0 .. split ])
}
