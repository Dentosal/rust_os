// Code style
#![forbid(private_in_public)]
#![deny(unused_assignments)]
// Safety
#![deny(overflowing_literals)]
#![deny(safe_packed_borrows)]
#![deny(unused_must_use)]

use bit_vec::BitVec;
use huffman_compress::{Book, Tree};

pub fn compress(book: Book<u8>, data: Vec<u8>) -> Vec<u8> {
    assert_eq!(book.len(), 0x100);

    let mut buffer = BitVec::new();
    for byte in data {
        book.encode(&mut buffer, &byte).unwrap();
    }

    buffer.to_bytes()
}

pub fn decompress(bittree: BitVec, frq_table: Vec<u8>, compressed: Vec<u8>) -> Vec<u8> {
    assert_eq!(bittree.len(), 511);
    assert_eq!(frq_table.len(), 256);

    Tree::from_bits(bittree, frq_table)
        .unwrap()
        .unbounded_decoder(&BitVec::from_bytes(&compressed))
        .collect()
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use std::iter::FromIterator;

    use bit_vec::BitVec;
    use huffman_compress::CodeBuilder;

    use super::{compress, decompress};

    fn check_compress_decompress_for(example_data: Vec<u8>, weight_scale_1: usize) {
        let mut weights: HashMap<u8, usize> = HashMap::new();
        for byte in example_data.clone() {
            *weights.entry(byte).or_insert(0) += weight_scale_1;
        }
        for byte in 0..=0xff {
            weights.entry(byte).or_insert(1);
        }

        let (book, tree) = CodeBuilder::from_iter(weights).finish();
        assert_eq!(book.len(), 0x100);

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
        let data = vec![];
        check_compress_decompress_for(data.clone(), 0);
        check_compress_decompress_for(data.clone(), 1);
        check_compress_decompress_for(data.clone(), 2);
        check_compress_decompress_for(data.clone(), 3);
        check_compress_decompress_for(data, 4);
    }

    #[test]
    fn test_compress_decompress_0() {
        let data = vec![0];
        check_compress_decompress_for(data.clone(), 0);
        check_compress_decompress_for(data.clone(), 1);
        check_compress_decompress_for(data.clone(), 2);
        check_compress_decompress_for(data.clone(), 3);
        check_compress_decompress_for(data, 4);
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
        check_compress_decompress_for(data.clone(), 0);
        check_compress_decompress_for(data.clone(), 1);
        check_compress_decompress_for(data.clone(), 2);
        check_compress_decompress_for(data.clone(), 3);
        check_compress_decompress_for(data, 4);
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
        check_compress_decompress_for(data.clone(), 0);
        check_compress_decompress_for(data.clone(), 1);
        check_compress_decompress_for(data.clone(), 2);
        check_compress_decompress_for(data.clone(), 3);
        check_compress_decompress_for(data, 4);
    }

    #[test]
    fn test_compress_decompress_3() {
        let data = vec![26, 120];
        check_compress_decompress_for(data.clone(), 0);
        check_compress_decompress_for(data.clone(), 1);
        check_compress_decompress_for(data.clone(), 2);
        check_compress_decompress_for(data.clone(), 3);
        check_compress_decompress_for(data, 4);
    }

    #[test]
    fn test_compress_decompress_4() {
        let data = vec![1; 1000];
        check_compress_decompress_for(data.clone(), 0);
        check_compress_decompress_for(data.clone(), 1);
        check_compress_decompress_for(data.clone(), 2);
        check_compress_decompress_for(data.clone(), 3);
        check_compress_decompress_for(data, 4);
    }

    #[test]
    fn test_compress_decompress_5() {
        let data = vec![
            25, 142, 222, 10, 73, 169, 72, 7, 148, 158, 10, 65, 105, 72, 81, 193, 210, 242, 167, 241, 65, 0, 77, 198,
            152, 224, 56, 39, 199, 5, 3, 131, 241, 193, 201, 44, 17, 239, 45, 43, 141, 48, 33, 192, 216, 224, 60, 56,
            63, 28, 28, 146, 193, 28, 169, 229, 165, 113, 166, 12, 56, 1, 14, 1, 1, 193, 50, 28, 28, 146, 193, 28, 213,
            255, 255, 255, 255, 255, 255, 1,
        ];
        check_compress_decompress_for(data.clone(), 0);
        check_compress_decompress_for(data.clone(), 1);
        check_compress_decompress_for(data.clone(), 2);
        check_compress_decompress_for(data.clone(), 3);
        check_compress_decompress_for(data, 4);
    }

    #[test]
    fn test_compress_decompress_6() {
        let data = vec![
            64, 167, 92, 137, 225, 229, 224, 56, 235, 85, 204, 141, 224, 185, 88, 231, 145, 107, 90, 176, 19, 105, 204,
            179, 202, 90, 204, 184, 45, 135, 55, 6, 243, 86, 28, 57, 229, 101, 112, 22, 229, 102, 195, 102, 108, 174,
            14, 60, 103, 194, 54, 177, 226, 198, 217, 99, 193, 105, 226, 206, 108, 195, 179, 30,
        ];
        check_compress_decompress_for(data.clone(), 0);
        check_compress_decompress_for(data.clone(), 1);
        check_compress_decompress_for(data.clone(), 2);
        check_compress_decompress_for(data.clone(), 3);
        check_compress_decompress_for(data, 4);
    }

    #[test]
    fn test_compress_decompress_7() {
        let data = vec![
            64, 167, 92, 137, 225, 229, 224, 56, 235, 85, 204, 141, 224, 185, 88, 231, 145, 107, 90, 176, 19, 105, 204,
            179, 202, 90, 204, 184, 45, 135, 55, 6, 243, 86, 28, 57, 229, 101, 112, 22, 229, 102, 195, 102, 108, 174,
            14, 60, 103, 194, 54, 177, 226, 198, 217, 99, 193, 105, 226, 206, 108, 195, 179, 30,
        ];
        check_compress_decompress_for(data.clone(), 0);
        check_compress_decompress_for(data.clone(), 1);
        check_compress_decompress_for(data.clone(), 2);
        check_compress_decompress_for(data.clone(), 3);
        check_compress_decompress_for(data, 4);
    }

    #[test]
    fn test_compress_decompress_8() {
        let data = vec![0; 100];
        check_compress_decompress_for(data.clone(), 0);
        check_compress_decompress_for(data.clone(), 1);
        check_compress_decompress_for(data.clone(), 2);
        check_compress_decompress_for(data.clone(), 3);
        check_compress_decompress_for(data, 4);
    }

    #[test]
    fn test_compress_decompress_9() {
        let data = vec![255; 10000];
        check_compress_decompress_for(data.clone(), 0);
        check_compress_decompress_for(data.clone(), 1);
        check_compress_decompress_for(data.clone(), 2);
        check_compress_decompress_for(data.clone(), 3);
        check_compress_decompress_for(data, 4);
    }

    #[test]
    fn test_full() {
        let bittree_bytes = [
            64, 167, 92, 137, 225, 229, 224, 56, 235, 85, 204, 141, 224, 185, 88, 231, 145, 107, 90, 176, 19, 105, 204,
            179, 202, 90, 204, 184, 45, 135, 55, 6, 243, 86, 28, 57, 229, 101, 112, 22, 229, 102, 195, 102, 108, 174,
            14, 60, 103, 194, 54, 177, 226, 198, 217, 99, 193, 105, 226, 206, 108, 195, 179, 30,
        ];

        let original_start = vec![
            102, 49, 192, 142, 208, 142, 216, 142, 192, 142, 224, 142, 232, 72, 188, 0, 128, 31, 0, 0, 0, 0, 0, 232, 1,
            18, 0, 0, 72, 184, 79, 79, 83, 79, 32, 79, 114, 79, 72, 137, 4, 37, 0, 128, 11, 0, 72, 184, 101, 79, 116,
            79, 117, 79, 114, 79, 72, 137, 4, 37, 8, 128, 11, 0, 72, 184, 110, 79, 101, 79, 100, 79, 33, 79, 72, 137,
            4, 37, 16, 128, 11, 0, 244, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];

        let compressed_start = vec![
            25, 142, 222, 10, 73, 169, 72, 7, 148, 158, 10, 65, 105, 72, 81, 193, 210, 242, 167, 241, 65, 0, 77, 198,
            152, 224, 56, 39, 199, 5, 3, 131, 241, 193, 201, 44, 17, 239, 45, 43, 141, 48, 33, 192, 216, 224, 60, 56,
            63, 28, 28, 146, 193, 28, 169, 229, 165, 113, 166, 12, 56, 1, 14, 1, 1, 193, 50, 28, 28, 146, 193, 28, 213,
            229, 165, 63, 127, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255,
        ];

        let weights_array: [usize; 0x100] = [
            72272, 4954, 3635, 1394, 3185, 758, 329, 450, 1011, 323, 281, 384, 636, 272, 215, 1634, 924, 346, 355, 324,
            224, 344, 146, 163, 609, 124, 111, 115, 293, 148, 646, 815, 1646, 213, 107, 108, 3060, 161, 102, 150, 478,
            316, 103, 147, 637, 450, 420, 947, 437, 570, 211, 133, 286, 377, 295, 161, 373, 391, 287, 170, 241, 413,
            153, 356, 512, 1478, 216, 299, 955, 440, 314, 352, 6016, 986, 123, 161, 1620, 291, 346, 289, 477, 147, 198,
            414, 390, 298, 288, 284, 218, 161, 96, 301, 327, 239, 274, 526, 244, 1062, 402, 728, 655, 1409, 588, 447,
            443, 1272, 90, 271, 1067, 441, 1326, 1081, 487, 132, 1455, 1238, 1829, 1139, 307, 324, 358, 290, 109, 156,
            323, 119, 161, 323, 742, 353, 132, 797, 600, 501, 124, 155, 1757, 3451, 1195, 1124, 881, 1726, 508, 108,
            941, 114, 119, 104, 137, 101, 113, 112, 106, 168, 106, 111, 103, 101, 95, 121, 132, 95, 93, 99, 126, 115,
            269, 364, 147, 111, 144, 265, 124, 106, 100, 81, 184, 133, 88, 100, 117, 98, 291, 238, 232, 239, 348, 97,
            157, 142, 289, 239, 749, 525, 163, 538, 286, 193, 293, 482, 196, 209, 158, 112, 191, 142, 194, 202, 212,
            184, 195, 158, 97, 106, 125, 157, 174, 125, 130, 157, 113, 85, 165, 293, 320, 215, 176, 123, 108, 142, 147,
            220, 1232, 553, 143, 508, 226, 169, 173, 231, 317, 124, 150, 198, 110, 107, 304, 309, 336, 184, 170, 262,
            322, 349, 844, 6317,
        ];

        let frq_table = vec![
            0, 114, 99, 209, 176, 56, 65, 128, 192, 72, 36, 53, 162, 161, 204, 5, 158, 90, 197, 11, 206, 212, 187, 84,
            255, 210, 200, 57, 181, 163, 243, 82, 179, 174, 131, 98, 207, 157, 149, 4, 76, 15, 31, 38, 156, 42, 147,
            61, 32, 83, 201, 50, 254, 46, 208, 213, 173, 154, 152, 33, 245, 34, 225, 137, 141, 14, 228, 143, 66, 35,
            122, 48, 88, 231, 136, 140, 69, 109, 104, 244, 169, 155, 26, 103, 203, 151, 20, 45, 7, 236, 220, 150, 145,
            165, 239, 2, 116, 16, 184, 27, 180, 183, 146, 125, 144, 47, 80, 191, 185, 68, 40, 93, 60, 199, 112, 159,
            227, 96, 74, 241, 172, 134, 73, 25, 217, 214, 164, 133, 8, 235, 142, 64, 251, 218, 160, 193, 95, 97, 130,
            113, 171, 177, 51, 166, 108, 111, 195, 107, 13, 233, 94, 148, 229, 139, 10, 205, 189, 87, 196, 117, 49, 52,
            234, 170, 58, 86, 190, 79, 121, 182, 77, 223, 198, 28, 22, 230, 168, 81, 102, 54, 43, 29, 138, 85, 67, 132,
            242, 39, 91, 24, 246, 118, 62, 135, 247, 232, 1, 115, 123, 219, 215, 188, 70, 211, 202, 41, 240, 12, 105,
            44, 224, 252, 126, 89, 75, 55, 127, 124, 30, 9, 119, 37, 194, 19, 100, 92, 23, 222, 6, 175, 221, 153, 110,
            248, 237, 250, 59, 238, 21, 78, 17, 186, 253, 3, 101, 216, 226, 71, 129, 18, 63, 120, 178, 106, 249, 167,
        ];

        let mut weights: HashMap<u8, usize> = HashMap::new();
        for byte in 0..=0xff {
            weights.insert(byte, weights_array[byte as usize]);
        }

        let (book_w, tree_w) = CodeBuilder::from_iter(weights).finish();
        assert_eq!(book_w.len(), 0x100);

        let mut buffer_w = BitVec::new();
        for byte in original_start.clone() {
            book_w.encode(&mut buffer_w, &byte).unwrap();
        }
        let compressed_w = buffer_w.to_bytes();

        let mut bittree = BitVec::from_bytes(&bittree_bytes);
        let _ = bittree.pop().unwrap();

        assert_eq!(bittree.len(), 511);
        assert_eq!(frq_table.len(), 256);

        let (bits_w, frqs_w) = tree_w.to_bits();

        assert_eq!(bits_w, bittree);
        assert_eq!(frqs_w, frq_table);

        // Compression

        let compressed_f = compress(book_w, original_start.to_vec());
        let len_test = compressed_f.len() - 1;
        assert!(len_test > 20);
        assert_eq!(compressed_f[..len_test], compressed_start[..len_test]);
        assert_eq!(compressed_f[..len_test], compressed_w[..len_test]);

        // Decompression

        let decompressed_f = decompress(bittree, frq_table, compressed_w);
        let len_test = decompressed_f.len() - 1;
        assert!(len_test > 20);
        assert_eq!(decompressed_f[..len_test], original_start[..len_test]);
    }
}
