//! Virtual filesystem

#![no_std]
#![feature(allocator_api)]
#![feature(no_more_cas)]
#![deny(unused_must_use)]

extern crate alloc;
extern crate libd7;

use alloc::vec::Vec;

use libd7::{ipc, process::ProcessId, select, syscall};

use fatfs::{Read, Write};

#[derive(Debug)]
enum DiskCursorIoError {
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

fn read_sectors(sector: u64, count: u8) -> Vec<u8> {
    ipc::request("ata_pio/drive/1/read", (sector, count)).expect("ata read")
}

struct DiskCursor {
    sector: u64,
    offset: usize,
}

impl DiskCursor {
    fn get_position(&self) -> usize {
        (self.sector * 0x200) as usize + self.offset
    }

    fn set_position(&mut self, position: usize) {
        self.sector = (position / 0x200) as u64;
        self.offset = position % 0x200;
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
            let data: Vec<u8> = read_sectors(self.sector, ((buf.len() - i) / 0x200).max(1) as u8);
            let data = &data[self.offset..];
            if data.len() == 0 {
                break;
            }
            let end = (i + data.len()).min(buf.len());
            let len = end - i;
            buf[i..end].copy_from_slice(&data[..len]);
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

        let start_is_exact = self.offset % 0x200 == 0;
        let end_is_exact = (self.offset + buf.len()) % 0x200 == 0;

        let logical_start = (self.sector * 0x200) as usize + self.offset;
        let logical_end = logical_start + buf.len();

        let first_sector = self.sector;
        let last_sector = (logical_end / 0x200) as u64;
        let single_sector = first_sector == last_sector;

        log::info!(
            "{:#?}",
            (
                buf,
                self.sector,
                self.offset,
                start_is_exact,
                end_is_exact,
                logical_start,
                logical_end,
                first_sector,
                last_sector,
                single_sector,
            )
        );

        let (head, tail) = if single_sector {
            if start_is_exact && end_is_exact {
                (Vec::new(), Vec::new())
            } else {
                let a = read_sectors(self.sector, 1);
                (a.clone(), a)
            }
        } else {
            (read_sectors(first_sector, 1), read_sectors(last_sector, 1))
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
            data.extend(&tail[logical_end % 0x200..]);
        }

        ipc::deliver("ata_pio/drive/1/write", &(self.sector, data)).expect("ata");

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

#[no_mangle]
fn main() -> ! {
    log::info!("daemon starting");

    // Storage backends
    // TODO: backend registers to us, instead of active waiting
    libd7::service::wait_for_one("driver_ata_pio");

    // Subscribe to client requests
    let server: ipc::Server<(), ()> = ipc::Server::exact("fatfs").unwrap();

    // Inform serviced that we are running.
    libd7::service::register("daemon_fatfs", false);

    log::info!("daemon running");

    let c = DiskCursor {
        sector: 0,
        offset: 0,
    };
    let fs = fatfs::FileSystem::new(c, fatfs::FsOptions::new()).expect("open fs");
    let mut cursor = fs.root_dir();

    let mut result = Vec::new();
    for entry in cursor.iter() {
        let entry = entry.expect("Entry");
        result.push(entry.file_name());
    }

    let mut cursor = fs.root_dir().open_dir("test/").expect("test/");

    let mut result = Vec::new();
    for entry in cursor.iter() {
        let entry = entry.expect("Entry");
        result.push(entry.file_name());
    }

    log::info!("{:?}", result);

    let mut file = fs.root_dir().create_file("example.txt").expect("open file");
    file.write(b"Example text").expect("Write failed");
    file.flush().expect("Close");

    todo!();
    // loop {
    //     select! {
    //         one(get_mac) => get_mac.handle(|()| Ok(device.mac_addr())).unwrap(),
    //     }
    // }
}
