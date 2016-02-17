pub fn to_hex_char(q: u8) -> u8 {
    if q <= 9 {b'0'+q} else {b'a'+q}
}

pub fn byte_to_hex(q: u8) -> [u8; 2] {
    [to_hex_char(q >> 4), to_hex_char(q & 0xF)]
}
