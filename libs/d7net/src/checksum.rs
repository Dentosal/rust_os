//// Standard internet checksum algorithm used by IPv4, ICMP, UDP and TCP.
/// Returns the result as "big-endian" [low, high] pair of u8's.
pub fn checksum_be<'a>(mut data: impl Iterator<Item = &'a u8>) -> [u8; 2] {
    let mut sum: u16 = 0;
    while let Some(&low) = data.next() {
        let &high = data.next().unwrap_or(&0);

        let word = ((high as u16) << 8) | (low as u16);
        let (new_sum, carry) = sum.overflowing_add(word);
        if carry {
            let (new_sum, carry) = sum.overflowing_add(word);
            sum = new_sum.checked_add(carry as u16).unwrap();
        } else {
            sum = new_sum;
        }
    }
    let inv = !sum;
    [inv as u8, (inv >> 8) as u8]
}

#[cfg(test)]
mod test {
    use super::*;

    /// https://www.thegeekstuff.com/2012/05/ip-header-checksum/
    #[rustfmt::skip]
    const EXAMPLE0: [u8; 20] = [
        0x45, 0x00, 0x00, 0x3c,
        0x1c, 0x46, 0x40, 0x00,
        0x40, 0x06, 0xb1, 0xe6, // 0xb1e6
        0xac, 0x10, 0x0a, 0x63,
        0xac, 0x10, 0x0a, 0x0c,
    ];

    #[rustfmt::skip]
    const EXAMPLE0_NO_CHECKSUM: [u8; 20] = [
        0x45, 0x00, 0x00, 0x3c,
        0x1c, 0x46, 0x40, 0x00,
        0x40, 0x06, 0x00, 0x00, // end of this line
        0xac, 0x10, 0x0a, 0x63,
        0xac, 0x10, 0x0a, 0x0c,
    ];

    #[test]
    fn test_checksum_example0() {
        let cs = checksum_be(EXAMPLE0.iter());
        assert_eq!(cs, [0, 0]);

        let cs = checksum_be(EXAMPLE0_NO_CHECKSUM.iter());
        assert_eq!(cs, [0xb1, 0xe6]);
    }


    /// https://www.thegeekstuff.com/2012/05/ip-header-checksum/
    #[rustfmt::skip]
    const EXAMPLE1: [u8; 32] = [
        // Pseudo header
        0x0a, 0x00, 0x02, 0x0f, // src ip
        0x0a, 0x00, 0x02, 0x02, // dst ip
        0x00,                   // zeros
        0x06,                   // protocol: TCP
        0x00, 0x14,             // TCP length: 20 bytes
        // TCP packet
        0x00, 0x16, 0xd4, 0x64, 0x4a, 0x51, 0x6c, 0xa4,
        0x00, 0x13, 0x88, 0x02, 0x50, 0x12, 0x22, 0x38,
        0x62, 0x18, 0x00, 0x00, // [0..2] (0x62, 0x18)
    ];

    #[rustfmt::skip]
    const EXAMPLE1_NO_CHECKSUM: [u8; 32] = [
        // Pseudo header
        0x0a, 0x00, 0x02, 0x0f, // src ip
        0x0a, 0x00, 0x02, 0x02, // dst ip
        0x00,                   // zeros
        0x06,                   // protocol: TCP
        0x00, 0x14,             // TCP length: 20 bytes
        // TCP packet
        0x00, 0x16, 0xd4, 0x64, 0x4a, 0x51, 0x6c, 0xa4,
        0x00, 0x13, 0x88, 0x02, 0x50, 0x12, 0x22, 0x38,
        0x00, 0x00, 0x00, 0x00, // [0..2]
    ];


    #[test]
    fn test_checksum_example1() {
        // let cs = checksum_be(EXAMPLE1.iter());
        // assert_eq!(cs, [0, 0]);

        let cs = checksum_be(EXAMPLE1_NO_CHECKSUM.iter());
        assert_eq!(cs, [0x62, 0x04]);
    }
}
