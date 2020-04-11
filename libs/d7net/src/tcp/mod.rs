mod segment;

pub use segment::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandshakeStep {
    Syn,
    SynAck,
    Ack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handshake;