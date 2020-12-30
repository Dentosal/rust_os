mod segment;

pub use segment::*;

/// https://en.wikipedia.org/wiki/Transmission_Control_Protocol#Protocol_operation
/// https://en.wikipedia.org/wiki/Transmission_Control_Protocol#/media/File:Tcp_state_diagram_fixed_new.svg
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionState {
    /// (server) waiting for a connection request from any remote TCP and port
    Listen,
    /// (client) waiting for a matching connection request after having sent a connection request
    SynSent,
    /// (server) waiting for a confirming connection request acknowledgment
    SynReceived,
    /// (both) an open connection, data received can be delivered to the user
    Established,
    /// (both) waiting for a connection termination (request|acknowledgment) from the remote TCP
    FinWait1,
    /// (both) waiting for a connection termination request from the remote TCP
    FinWait2,
    /// (both) waiting for a connection termination request from the local user
    CloseWait,
    /// (both) waiting for a connection termination request acknowledgment from the remote TCP
    Closing,
    /// (both) waiting for an acknowledgment of a sent connection termination request
    LastAck,
    /// (either) waiting for enough time to pass to be sure the remote TCP received the acknowledgment of its connection termination request.
    TimeWait,
    /// (both) no connection state at all
    Closed,
}
