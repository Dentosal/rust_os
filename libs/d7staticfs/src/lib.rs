#![no_std]

pub const SECTOR_SIZE: u64 = 0x200;

pub const MBR_POSITION: u16 = 0x01fa;
pub const HEADER_MAGIC: u32 = 0xd7cafed7;
pub const ONLY_VERSION: u32 = 0x00000001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct FileEntry {
    pub name: [u8; 12],
    pub size: u32,
}
impl FileEntry {
    pub fn new_skip(size: u32) -> Self {
        Self {
            name: [0; 12],
            size,
        }
    }

    pub fn new_zero() -> Self {
        Self {
            name: [0; 12],
            size: 0,
        }
    }

    pub fn new(name: &str, size: u32) -> Self {
        let mut name_buf = [0; 12];

        assert!(name.bytes().len() > 0);
        assert!(name.bytes().len() <= 12);

        for (i, c) in name.bytes().take(12).enumerate() {
            name_buf[i] = c;
        }

        Self {
            name: name_buf,
            size,
        }
    }

    pub fn from_bytes(b: [u8; 16]) -> Self {
        let mut name = [0; 12];
        let mut size = [0; 4];

        for i in 0..12 {
            name[i] = b[i];
        }

        for i in 0..4 {
            size[i] = b[12 + i];
        }

        Self {
            name,
            size: u32::from_le_bytes(size),
        }
    }

    pub fn to_bytes(&self) -> [u8; 16] {
        let mut result = [0; 16];

        for i in 0..12 {
            result[i] = self.name[i];
        }

        for (i, &b) in self.size.to_le_bytes().iter().enumerate() {
            result[12 + i] = b;
        }

        result
    }

    pub fn is_skip(&self) -> bool {
        self.name == [0; 12]
    }

    pub fn is_zero(&self) -> bool {
        self.is_skip() && self.size == 0
    }

    pub fn name_matches(&self, name: &str) -> bool {
        let mut trimmed: &[u8] = &self.name;
        while trimmed.len() > 0 && trimmed[trimmed.len()-1] == 0 {
            trimmed = &trimmed[..trimmed.len()-1];
        }
        trimmed == name.as_bytes()
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_file_entry() {
        use super::FileEntry;

        let name = "Test_File";
        let size = 0x1234u32;


        let mut byte_buffer: [u8; 16] = [0; 16];
        for (i, c) in name.bytes().enumerate() {
            byte_buffer[i] = c;
        }
        for (i, &c) in size.to_le_bytes().iter().enumerate() {
            byte_buffer[12 + i] = c;
        }

        let fe = FileEntry::from_bytes(byte_buffer);

        assert_eq!(fe, FileEntry::new(name, size));

        assert!(fe.name_matches(name));
        assert!(!fe.name_matches(""));
        assert!(!fe.name_matches("Test_File2"));
        assert!(!fe.name_matches("Test_Fil"));

        assert_eq!(fe, FileEntry::from_bytes(fe.to_bytes()));
    }
}
