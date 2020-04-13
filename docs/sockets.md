# Network sockets

Socket is created by writing a create request `/srv/net/newsocket`, an endpoint operated by `netd`.
`netd` then creates the requested socket under `/srv/net/socket/$socketname` and returns  `socketname`.
The creation protocol can be found under [`d7protocol`](../libs/d7protocol/README.md).

After this the socketname can be used to send and receive packets.