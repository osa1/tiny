pub fn find_char(chrs : &[char], chr0 : char) -> Option<usize> {
    for (chr_idx, chr) in chrs.iter().enumerate() {
        if *chr == chr0 {
            return Some(chr_idx);
        }
    }
    None
}

pub fn find_byte(buf : &[u8], byte0 : u8) -> Option<usize> {
    for (byte_idx, byte) in buf.iter().enumerate() {
        if *byte == byte0 {
            return Some(byte_idx);
        }
    }
    None
}
