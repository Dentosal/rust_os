// Code style
#![forbid(private_in_public)]
#![deny(unused_assignments)]
// Safety
#![deny(overflowing_literals)]
#![deny(safe_packed_borrows)]
#![deny(unused_must_use)]

use bit_vec::BitVec;

use super::{Book, Tree};

#[cfg(feature = "use-std")]
pub fn compress(book: Book, data: Vec<u8>) -> Vec<u8> {
    assert_eq!(book.len(), 0x100);

    let mut buffer = BitVec::new();
    for byte in data {
        book.encode(&mut buffer, &byte).unwrap();
    }

    // Append "number of used bits in the last byte" to the data
    let mut last_byte_bits = (buffer.len() % 8) as u8;
    if last_byte_bits == 0 {
        last_byte_bits = 8;
    }
    let mut result = buffer.to_bytes();
    result.push(last_byte_bits);
    result
}

#[cfg(feature = "use-std")]
pub fn decompress(bittree: BitVec, frq_table: &[u8], compressed: &[u8]) -> Vec<u8> {
    assert_eq!(bittree.len(), 511);
    assert_eq!(frq_table.len(), 256);

    let count = compressed.len() - 1;
    let last_byte_bits = *compressed.last().expect("Compressed data empty");
    assert!(last_byte_bits != 0 && last_byte_bits <= 8);
    let mut bits = BitVec::from_bytes(&compressed[..count]);
    for _ in 0..(8 - last_byte_bits) {
        let bit = bits.pop();
        assert_eq!(bit, Some(false));
    }

    Tree::from_bits(bittree, frq_table)
        .unwrap()
        .unbounded_decoder(&bits)
        .collect()
}

#[cfg(feature = "use-std")]
pub fn build_code(data: Vec<u8>) -> (Book, Tree) {
    use huffman_compress::CodeBuilder;
    use std::collections::HashMap;
    use std::iter::FromIterator;

    let mut weights: HashMap<u8, usize> = HashMap::new();
    for byte in data.clone() {
        *weights.entry(byte).or_insert(0) += 1;
    }
    for byte in 0..=0xff {
        weights.entry(byte).or_insert(1);
    }

    let (book, tree) = CodeBuilder::from_iter(weights).finish();
    assert_eq!(book.len(), 0x100);
    (book, tree)
}

#[cfg(test)]
mod test {
    use bit_vec::BitVec;

    use super::{build_code, compress, decompress};

    #[test]
    fn check_bitvec() {
        let mut bv = BitVec::new();
        assert_eq!(bv.to_bytes(), vec![]);
        bv.push(true);
        assert_eq!(bv.to_bytes(), vec![0b1000_0000]);
        for _ in 0..7 {
            bv.push(true);
        }
        assert_eq!(bv.to_bytes(), vec![0b1111_1111]);
        bv.push(true);
        assert_eq!(bv.to_bytes(), vec![0b1111_1111, 0b1000_0000]);
    }

    fn check_compress_decompress_for(example_data: Vec<u8>) {
        let (book, tree) = build_code(example_data.clone());
        let compressed = compress(book.clone(), example_data.clone());

        let (bittree, frq_table) = tree.to_bits();
        assert_eq!(bittree.len(), 511);

        let decompressed_origtree: Vec<u8> = tree.unbounded_decoder(&BitVec::from_bytes(&compressed)).collect();
        assert_eq!(decompressed_origtree[..example_data.len()], example_data[..]);

        let decompressed = decompress(bittree, frq_table, compressed);
        assert_eq!(decompressed[..example_data.len()], example_data[..]);
    }

    #[test]
    fn test_compress_decompress_empty() {
        check_compress_decompress_for(vec![]);
    }

    #[test]
    fn test_compress_decompress_0() {
        check_compress_decompress_for(vec![0]);
    }

    #[test]
    fn test_compress_decompress_1() {
        let data = vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 106, 247, 234, 70, 134,
            165, 22, 48, 85, 21, 108, 231, 241, 152, 12, 161, 31, 208, 24, 131, 175, 93, 205, 64, 22, 225, 158, 238,
            90, 42, 185, 182, 94, 33, 4, 73, 65, 231, 209, 85, 27, 100, 146, 139, 33, 78, 219, 86, 115, 1, 223, 239,
            125, 83, 204, 45, 219, 103, 156, 195, 102, 37, 174, 124, 225, 96, 52, 186, 206, 251, 125, 187, 251, 34,
            203, 18, 73, 92, 222, 13, 160, 99, 169, 95, 177, 95, 70, 166, 80, 148, 203, 104, 74, 157, 80, 80, 66, 60,
            26, 120,
        ];
        check_compress_decompress_for(data);
    }

    #[test]
    fn test_compress_decompress_2() {
        let data = vec![
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 106, 247, 234, 70, 134,
            165, 22, 48, 85, 21, 108, 231, 241, 152, 12, 161, 31, 208, 24, 131, 175, 93, 205, 64, 22, 225, 158, 238,
            90, 42, 185, 182, 94, 33, 4, 73, 65, 231, 209, 85, 27, 100, 146, 139, 33, 78, 219, 86, 115, 1, 223, 239,
            125, 83, 204, 45, 219, 103, 156, 195, 102, 37, 174, 124, 225, 96, 52, 186, 206, 251, 125, 187, 251, 34,
            203, 18, 73, 92, 222, 13, 160, 99, 169, 95, 177, 95, 70, 166, 80, 148, 203, 104, 74, 157, 80, 80, 66, 60,
            26, 120,
        ];
        check_compress_decompress_for(data);
    }

    #[test]
    fn test_compress_decompress_3() {
        let data = vec![26, 120];
        check_compress_decompress_for(data);
    }

    #[test]
    fn test_compress_decompress_4() {
        let data = vec![1; 1000];
        check_compress_decompress_for(data);
    }

    #[test]
    fn test_compress_decompress_5() {
        let data = vec![
            25, 142, 222, 10, 73, 169, 72, 7, 148, 158, 10, 65, 105, 72, 81, 193, 210, 242, 167, 241, 65, 0, 77, 198,
            152, 224, 56, 39, 199, 5, 3, 131, 241, 193, 201, 44, 17, 239, 45, 43, 141, 48, 33, 192, 216, 224, 60, 56,
            63, 28, 28, 146, 193, 28, 169, 229, 165, 113, 166, 12, 56, 1, 14, 1, 1, 193, 50, 28, 28, 146, 193, 28, 213,
            255, 255, 255, 255, 255, 255, 1,
        ];
        check_compress_decompress_for(data);
    }

    #[test]
    fn test_compress_decompress_6() {
        let data = vec![
            64, 167, 92, 137, 225, 229, 224, 56, 235, 85, 204, 141, 224, 185, 88, 231, 145, 107, 90, 176, 19, 105, 204,
            179, 202, 90, 204, 184, 45, 135, 55, 6, 243, 86, 28, 57, 229, 101, 112, 22, 229, 102, 195, 102, 108, 174,
            14, 60, 103, 194, 54, 177, 226, 198, 217, 99, 193, 105, 226, 206, 108, 195, 179, 30,
        ];
        check_compress_decompress_for(data);
    }

    #[test]
    fn test_compress_decompress_7() {
        let data = vec![
            64, 167, 92, 137, 225, 229, 224, 56, 235, 85, 204, 141, 224, 185, 88, 231, 145, 107, 90, 176, 19, 105, 204,
            179, 202, 90, 204, 184, 45, 135, 55, 6, 243, 86, 28, 57, 229, 101, 112, 22, 229, 102, 195, 102, 108, 174,
            14, 60, 103, 194, 54, 177, 226, 198, 217, 99, 193, 105, 226, 206, 108, 195, 179, 30,
        ];
        check_compress_decompress_for(data);
    }

    #[test]
    fn test_compress_decompress_8() {
        let data = vec![0; 100];
        check_compress_decompress_for(data);
    }

    #[test]
    fn test_compress_decompress_9() {
        let data = vec![255; 10000];
        check_compress_decompress_for(data);
    }

    #[test]
    #[should_panic]
    fn test_fail_decompress_empty() {
        let (_book, tree) = build_code(vec![]);
        let (bittree, frq_table) = tree.to_bits();
        decompress(bittree, frq_table, vec![]);
    }

    #[test]
    #[should_panic]
    fn test_fail_decompress_only_one() {
        let (_book, tree) = build_code(vec![]);
        let (bittree, frq_table) = tree.to_bits();
        decompress(bittree, frq_table, vec![1]);
    }
}
