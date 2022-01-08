//! Kernel-provided signatures.
//! We expose a new submodule for each use case, so arguments
//! can be typed correctly and given separate context.

use crate::random;

use spin::Once;

use ed25519_dalek::Keypair;

static KEYPAIR: Once<Keypair> = Once::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidSignature;

#[cfg(not(test))]
#[allow(const_item_mutation)]
fn get_keypair() -> &'static Keypair {
    KEYPAIR.call_once(|| Keypair::generate(&mut random::KRNG))
}

#[cfg(test)]
fn get_keypair() -> &'static Keypair {
    use rand::rngs::OsRng;
    let mut csprng = OsRng {};
    KEYPAIR.call_once(|| Keypair::generate(&mut csprng))
}

pub mod capability {
    use ed25519_dalek::{Digest, Sha512, Signature};

    use super::{get_keypair, InvalidSignature};

    const SIGNATURE_CONTEXT: &[u8] = b"d7os-kernel-capability";

    pub fn sign(pid: u64, cap: u64) -> Signature {
        let mut prehashed: Sha512 = Sha512::new();
        prehashed.update(&pid.to_le_bytes());
        prehashed.update(&cap.to_le_bytes());
        get_keypair()
            .sign_prehashed(prehashed, Some(SIGNATURE_CONTEXT))
            .expect("Signing failed")
    }

    pub fn verify(pid: u64, cap: u64, signature: &Signature) -> Result<(), InvalidSignature> {
        let mut prehashed: Sha512 = Sha512::new();
        prehashed.update(&pid.to_le_bytes());
        prehashed.update(&cap.to_le_bytes());
        get_keypair()
            .verify_prehashed(prehashed, Some(SIGNATURE_CONTEXT), signature)
            .map_err(|_| InvalidSignature)
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::Signature;

    use super::capability;

    #[test]
    fn test_signature() {
        let s = capability::sign(5, 10);
        println!("{:?}", s);
        capability::verify(5, 10, &s).expect("Verify");
        assert!(capability::verify(6, 10, &s).is_err());

        let mut bytes = s.to_bytes();
        bytes[0] = bytes[0].wrapping_add(1);
        let s2 = Signature::new(bytes);

        assert!(capability::verify(6, 10, &s2).is_err());
    }
}
