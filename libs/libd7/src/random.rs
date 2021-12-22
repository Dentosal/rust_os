use crate::syscall::random;

/// Read cryptocraphically secure randomness from the kernel entropy pool
pub fn crypto(buffer: &mut [u8]) {
    for c in buffer.chunks_mut(8) {
        let b = random(0).to_le_bytes();
        c.copy_from_slice(&b[..c.len()]);
    }
}

/// Read cryptocraphically secure randomness from the kernel entropy pool
pub fn crypto_arr<const LEN: usize>() -> [u8; LEN] {
    let mut arr = [0u8; LEN];
    crypto(&mut arr);
    arr
}

/// Get a random number quickly (not cryptographically secure)
pub fn fast(buffer: &mut [u8]) {
    crypto(buffer); // TODO: speedup with userspace RDRAND and/or PRNG
}

/// Get a random number quickly (not cryptographically secure)
pub fn fast_arr<const LEN: usize>() -> [u8; LEN] {
    let mut arr = [0u8; LEN];
    fast(&mut arr);
    arr
}
