/// A rare byte used for escaping
pub const ESCAPE_BYTE: u8 = 0xaa;

/// Only escape after this many repetitions
const TRESHOLD: usize = 10;

pub fn compress(data: Vec<u8>) -> Vec<u8> {
    let mut result = Vec::new();

    let mut i: usize = 0;
    while i < data.len() {
        let byte = data[i];
        let mut c: usize = 0;
        while i + c < data.len() && data[i + c] == byte {
            c += 1;
            if c == std::u32::MAX as usize {
                break;
            }
        }

        if byte == ESCAPE_BYTE || c >= TRESHOLD {
            let z = (c as u32).to_le_bytes();
            result.push(ESCAPE_BYTE);
            result.push(byte);
            result.push(z[0]);
            result.push(z[1]);
            result.push(z[2]);
            result.push(z[3]);
        } else {
            for _ in 0..c {
                result.push(byte);
            }
        }

        i += c;
    }

    result
}

pub fn decompress(compressed: Vec<u8>) -> Vec<u8> {
    let mut result = Vec::new();
    let mut iter = compressed.into_iter();
    while let Some(byte) = iter.next() {
        if byte == ESCAPE_BYTE {
            let b = iter.next().expect("RLE decompression error");
            let zb0 = iter.next().expect("RLE decompression error");
            let zb1 = iter.next().expect("RLE decompression error");
            let zb2 = iter.next().expect("RLE decompression error");
            let zb3 = iter.next().expect("RLE decompression error");
            let count = u32::from_le_bytes([zb0, zb1, zb2, zb3]);
            assert_ne!(count, 0);
            for _ in 0..count {
                result.push(b);
            }
        } else {
            result.push(byte);
        }
    }

    result
}

#[cfg(test)]
mod test {
    use super::{compress, decompress, ESCAPE_BYTE};

    fn check_compress_decompress_for(example_data: Vec<u8>) {
        let compressed = compress(example_data.clone());
        let decompressed = decompress(compressed);
        assert_eq!(decompressed, example_data);
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
    fn test_compress_decompress_10() {
        let data = vec![ESCAPE_BYTE];
        check_compress_decompress_for(data);
    }

    #[test]
    fn test_compress_decompress_11() {
        let data = vec![ESCAPE_BYTE; 256];
        check_compress_decompress_for(data);
    }

    #[test]
    fn test_compress_decompress_12() {
        let data = vec![ESCAPE_BYTE; 1000];
        check_compress_decompress_for(data);
    }

    #[test]
    fn test_compress_escape() {
        assert_eq!(compress(vec![ESCAPE_BYTE]), vec![ESCAPE_BYTE, ESCAPE_BYTE, 1, 0, 0, 0]);
        assert_eq!(
            compress(vec![ESCAPE_BYTE, ESCAPE_BYTE]),
            vec![ESCAPE_BYTE, ESCAPE_BYTE, 2, 0, 0, 0]
        );
    }
}
