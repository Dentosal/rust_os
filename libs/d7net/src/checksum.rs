/// Standard internet checksum
pub fn inet_checksum(data: &[u8]) -> u16 {
    let mut result: u16 = 0;
    for chunk in data.chunks(2) {
        let v = u16::from_be_bytes([chunk[0], if chunk.len() == 2 { chunk[1] } else { 0 }]);
        let (r, c) = result.overflowing_add(v);
        result = r;
        if c {
            let (r, c) = result.overflowing_add(1);
            result = r;
            if c {
                result += 1;
            }
        }
    }
    !result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inet_checksum() {
        let ipv4_header = vec![
            0x45, 0x00, 0x00, 0x73, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, // Checksum start
            0x00, 0x00, // Checksum end
            0xc0, 0xa8, 0x00, 0x01, 0xc0, 0xa8, 0x00, 0xc7,
        ];
        let cksm = inet_checksum(&ipv4_header);
        println!("{:04x} {:04x}", cksm, 0xb861);
        assert_eq!(cksm, 0xb861);
    }
}
