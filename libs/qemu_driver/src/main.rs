#![feature(async_await)]

use futures_util::stream::StreamExt;
use std::env;
use std::io::*;
use std::process::Stdio;
use tokio::codec::{FramedRead, LinesCodec};
use tokio::prelude::*;
use tokio::process::{Child, Command};

const USAGE: &str = "args: config_file";

struct Qemu {
    pub process: Child,
}
impl Qemu {
    fn new(disk_file: &str) -> Self {
        let cmd: &str = &vec![
            "qemu-system-x86_64.exe",
            "-m",
            "4G",
            "-no-reboot",
            "-no-shutdown",
            "-drive",
            &format!("file={},format=raw,if=ide", disk_file),
            "-monitor",
            "stdio",
            "-s",
            "-S",
        ]
        .join(" ");

        let process = Command::new("sh")
            .args(&["-c", cmd])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Unable to start qemu");

        Self { process }
    }

    fn terminate(mut self) {
        {
            self.process
                .stdin()
                .as_mut()
                .unwrap()
                .write_all(b"q\n")
                .unwrap();
        }
        let ecode = self.process.wait().expect("failed to wait on child");
        assert!(ecode.success());
    }
}

struct Gdb {
    pub process: Child,
    stdout: FramedRead<Vec<u8>, LinesCodec>,
}
impl Gdb {
    fn new() -> Self {
        let process = Command::new("gdb")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            // .stderr(Stdio::null())
            .spawn()
            .expect("Unable to start gdb");

        let stdout = process.stdout().take().unwrap();
        Self {
            process,
            stdout: FramedRead::new(stdout, LinesCodec::new()),
        }
    }

    fn read(&mut self) -> Vec<u8> {
        let mut result = Vec::new();
        self.process
            .stdout
            .as_mut()
            .unwrap()
            .read_to_end(&mut result)
            .unwrap();
        result
    }

    fn write(&mut self, bytes: &[u8]) {
        self.process
            .stdin
            .as_mut()
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }

    fn start(&mut self) {}

    fn terminate(mut self) {
        self.write(b"q\n");
        let ecode = self.process.wait().expect("failed to wait on child");
        assert!(ecode.success());
    }
}

#[tokio::main]
async fn main() {
    let _args: Vec<_> = env::args().skip(1).collect();

    let mut qemu = Qemu::new("build/test_disk.img");
    let mut gdb = Gdb::new();

    gdb.start();

    std::thread::sleep_ms(1000);

    gdb.terminate();
    qemu.terminate();

    println!("DONE")
}
