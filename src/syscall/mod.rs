use multitasking::{ProcessId, PROCMAN, SCHEDULER};

pub struct SysCallResult {
    pub success: bool,
    pub result: u64,
}
impl SysCallResult {
    const fn ok(result: u64) -> SysCallResult {
        SysCallResult {
            success: true,
            result,
        }
    }

    const fn err(result: u64) -> SysCallResult {
        SysCallResult {
            success: false,
            result,
        }
    }
}

pub fn sc_exit(pid: ProcessId, status_code: u64) -> Option<SysCallResult> {
    PROCMAN.update(|pm| pm.kill(pid, status_code));
    None
}

/// None request that the next process will be scheduled
pub fn call(routine: u64, args: (u64, u64, u64, u64)) -> Option<SysCallResult> {
    match routine {
        // Exit
        0x00 => {
            if let Some(pid) = SCHEDULER.get_running_pid() {
                sc_exit(pid, args.0)
            } else {
                panic!("SysCall: exit: No process currently running");
            }
        }
        1 => Some(SysCallResult::ok(args.0 + args.1)), // sum
        _ => {
            panic!("TODO: handle invalid routine");
        } // invalid routine
    }
}
