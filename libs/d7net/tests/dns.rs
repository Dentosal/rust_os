use std::net::UdpSocket;

use d7net::dns;

#[test]
fn test_udp_dns() -> std::io::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("1.1.1.1:53")?;

    // A real domain

    let q = dns::make_question(1, "wikipedia.org", dns::QueryType::A);

    socket.send(&q)?;

    let mut buf = [0; 1024];
    let (n, _src) = socket.recv_from(&mut buf)?;
    let buf = &mut buf[..n];

    let reply = dns::parse_reply(&buf).expect("Resolution error");
    assert_eq!(reply.req_id, 1);
    assert!(reply.records.expect("No such domain").len() >= 1);

    // Nonexistent domain

    let q = dns::make_question(1, "this-domain.does-not.exist.local", dns::QueryType::A);

    socket.send(&q)?;

    let mut buf = [0; 1024];
    let (n, _src) = socket.recv_from(&mut buf)?;
    let buf = &mut buf[..n];

    let reply = dns::parse_reply(&buf).expect("Resolution error");
    assert!(reply.records.is_err());

    Ok(())
}
