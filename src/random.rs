use core::arch::asm;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

static ENTROPY_POOL: [AtomicU64; 512] = [const { AtomicU64::new(0) }; 512];
static READ_INDEX: AtomicUsize = AtomicUsize::new(0);
static WRITE_INDEX: AtomicUsize = AtomicUsize::new(0);

fn read_best_hw_random() -> Option<u64> {
    let (has_rdseed, has_rdrand) = crate::cpuid::supports_rdrand();

    if has_rdseed {
        return Some(unsafe { rdseed() });
    }

    if has_rdrand {
        return Some(unsafe { rdrand() });
    }

    None
}

/// Safety: Caller must ensure that rdseed instruction is available
unsafe fn rdseed() -> u64 {
    let retries: u64 = 10;
    let retries_left: u64;
    let rax: u64;
    asm!(r#"
        2:
            rdseed rax
            jc 3f
            loop 2b
        3:
        "#,
        inlateout("ecx") retries => retries_left,
        lateout("rax") rax,
        options(nomem, nostack)
    );

    if retries_left == 0 {
        panic!("rdseed failed");
    }

    rax
}

/// Safety: Caller must ensure that rdseed instruction is available
unsafe fn rdrand() -> u64 {
    let retries: u64 = 10;
    let retries_left: u64;
    let rax: u64;
    asm!(r#"
        2:
            rdrand rax
            jc 3f
            loop 2b
        3:
        "#,
        inlateout("ecx") retries => retries_left,
        lateout("rax") rax,
        options(nomem, nostack)
    );

    if retries_left == 0 {
        panic!("rdseed failed");
    }

    rax
}

fn push_seed(value: u64) {
    let index = WRITE_INDEX.fetch_add(1, Ordering::Relaxed);
    let index = index % ENTROPY_POOL.len();
    ENTROPY_POOL[index].fetch_xor(value, Ordering::Relaxed);
}

// Get some entropy, quickly and possibly not well
fn fast_entropy() -> u64 {
    let mut v = crate::driver::tsc::read();
    v = v.rotate_left(READ_INDEX.load(Ordering::Relaxed) as u32);

    if let Some(r) = read_best_hw_random() {
        v ^= r;
    }

    v
}

/// Insert some entropy
pub fn insert_entropy(v: u64) {
    let a = fast_entropy();
    let b = fast_entropy();
    push_seed((v ^ a).rotate_left(b as u32));
}

/// Read from the entropy pool
fn read_one() -> u64 {
    insert_entropy(0); // Do some fast-seeding

    let index = READ_INDEX.fetch_add(1, Ordering::Relaxed);
    let index = index % ENTROPY_POOL.len();

    // Adds 1 to the value to make sure same number cannot be read twice in low-entroy situations
    ENTROPY_POOL[index].fetch_add(1, Ordering::Relaxed)
}

/// Read one item from the entropy pool and hash it.
/// All randomness that is read from this module goes through this function.
/// It should be reasonably secure against entropy poisoning.
pub fn read() -> u64 {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(read_one().to_le_bytes());
    let hash = hasher.finalize();
    let mut result = [0u8; 8];
    result.copy_from_slice(&hash.as_slice()[..8]);
    u64::from_le_bytes(result)
}

#[derive(Debug, Clone, Copy)]
pub struct KernelRng;
impl rand_core::RngCore for KernelRng {
    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    fn next_u64(&mut self) -> u64 {
        read()
    }

    fn fill_bytes(&mut self, buffer: &mut [u8]) {
        for c in buffer.chunks_mut(8) {
            let b = read().to_le_bytes();
            c.copy_from_slice(&b[..c.len()]);
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}
impl rand_core::CryptoRng for KernelRng {}

pub const KRNG: KernelRng = KernelRng;

/// Initial pool seeding
pub fn init() {
    for _ in 0..(ENTROPY_POOL.len() * 5) {
        insert_entropy(0);
    }

    log::debug!("Random init done");
}
