//! The kernel entropy pool
//!
//! TODO: seed from RDRAND if available
//! TODO: seed from timer
//! TODO: seed from keyboard events
//!
//! TODO: a proper implementation
//! TOOD: a proper crypt-rng implementation,
//! maybe: https://en.wikipedia.org/wiki/Fortuna_(PRNG)

use spin::Mutex;

use rand::rngs::SmallRng;
use rand::{Rng, RngCore, SeedableRng};

lazy_static::lazy_static! {
    static ref RNG: Mutex<SmallRng> = Mutex::new(SmallRng::seed_from_u64({
        let low: u64; // only lower 32 bits are set
        let high: u64; // only lower 32 bits are set
        unsafe {
            // https://en.wikipedia.org/wiki/Time_Stamp_Counter
            asm!("rdtsc" : "={rdx}"(high)  "={rax}"(low) ::: "intel")
        }
        ((high as u64) << 32) | (low as u64)
    }));
}

pub fn fill_bytes(dest: &mut [u8]) {
    let mut r = RNG.try_lock().unwrap();
    r.fill_bytes(dest);
}
