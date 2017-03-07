pub struct SyscallResult {
    pub success: bool,
    pub result: u64
}
impl SyscallResult {
    const fn new(success: bool, result: u64) -> SyscallResult {
        SyscallResult { success: success, result: result }
    }
}

pub fn call(routine: u64, args: (u64, u64, u64, u64)) -> SyscallResult {
    match routine {
        0 => {
            //Process.kill(processid);
            SyscallResult::new(true, 0)
        },
        1 => SyscallResult::new(true, args.0+args.1), // sum
        _ => {panic!("TODO: handle invalid routine");} // invalid routine
    }
}
