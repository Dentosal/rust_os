use alloc::vec::Vec;

use crate::cache::DiskAccess;

#[derive(Debug)]
pub enum DiskCursorIoError {
    UnexpectedEof,
    WriteZero,
}
impl fatfs::IoError for DiskCursorIoError {
    fn is_interrupted(&self) -> bool {
        false
    }

    fn new_unexpected_eof_error() -> Self {
        Self::UnexpectedEof
    }

    fn new_write_zero_error() -> Self {
        Self::WriteZero
    }
}

pub struct DiskCursor {
    disk: DiskAccess,
    sector: u64,
    offset: usize,
}

impl DiskCursor {
    pub fn new(disk: DiskAccess) -> Self {
        Self {
            disk,
            sector: 0,
            offset: 0,
        }
    }

    fn get_position(&self) -> usize {
        (self.sector as usize) * self.disk.sector_size() + self.offset
    }

    fn set_position(&mut self, position: usize) {
        self.sector = (position / self.disk.sector_size()) as u64;
        self.offset = position % self.disk.sector_size();
    }

    fn move_cursor(&mut self, amount: usize) {
        self.set_position(self.get_position() + amount)
    }
}

impl fatfs::IoBase for DiskCursor {
    type Error = DiskCursorIoError;
}

impl fatfs::Read for DiskCursor {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, DiskCursorIoError> {
        let mut i = 0;
        while i < buf.len() {
            let data = self.disk.read(self.sector);
            let end = (i + data.len()).min(buf.len());
            let len = end - i;
            buf[i..end].copy_from_slice(&data[self.offset..self.offset + len]);
            i += len;
            self.move_cursor(i);
        }
        Ok(i)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), DiskCursorIoError> {
        let n = self.read(buf)?;
        assert!(n == buf.len(), "TODO: Error");
        Ok(())
    }
}

impl fatfs::Write for DiskCursor {
    fn write(&mut self, buf: &[u8]) -> Result<usize, DiskCursorIoError> {
        assert!(buf.len() != 0);

        let start_is_exact = self.offset % self.disk.sector_size() == 0;
        let end_is_exact = (self.offset + buf.len()) % self.disk.sector_size() == 0;

        let logical_start = (self.sector as usize) * self.disk.sector_size() + self.offset;
        let logical_end = logical_start + buf.len();

        let first_sector = self.sector;
        let last_sector = (logical_end / self.disk.sector_size()) as u64;
        let single_sector = first_sector == last_sector;

        let (head, tail) = if single_sector {
            if start_is_exact && end_is_exact {
                (Vec::new(), Vec::new())
            } else {
                let a = self.disk.read(self.sector);
                (a.clone(), a)
            }
        } else {
            (self.disk.read(first_sector), self.disk.read(last_sector))
        };

        // Optimization: don't write if already written
        if single_sector && &head[self.offset..self.offset + buf.len()] == buf {
            log::trace!(
                "write {:?} {:?} optimized away",
                buf,
                (self.sector, self.offset)
            );
        }

        let mut data = head[..self.offset].to_vec();
        data.extend(buf);
        if !end_is_exact {
            data.extend(&tail[logical_end % self.disk.sector_size()..]);
        }

        for (i, block) in data.chunks_exact(self.disk.sector_size()).enumerate() {
            self.disk.write(first_sector + (i as u64), block.to_vec());
        }

        self.move_cursor(buf.len());
        Ok(buf.len())
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<(), DiskCursorIoError> {
        self.write(buf)?;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DiskCursorIoError> {
        Ok(())
    }
}

impl fatfs::Seek for DiskCursor {
    fn seek(&mut self, pos: fatfs::SeekFrom) -> Result<u64, DiskCursorIoError> {
        match pos {
            fatfs::SeekFrom::Start(i) => {
                self.set_position(i as usize);
                Ok(i)
            },
            fatfs::SeekFrom::End(i) => {
                todo!("Seek from end")
            },
            fatfs::SeekFrom::Current(i) => {
                let new_pos = (self.get_position() as i64) + i;
                self.set_position(new_pos as usize);
                Ok(new_pos as u64)
            },
        }
    }
}
