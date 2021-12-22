# `netd` - Networking daemon

Endpoints

* `netd/newsocket/udp` (host:port) → socketid - Reserves a new UDP socket (bind)
* `netd/newsocket/tcp` (host:port) → socketid - Reserves a new TCP socket (bind)
* `netd/udp/recv/:id` - Subscription point for incoming packets
* `netd/tcp/recv/:id` - Subscription point for incoming data
* `netd/udp/send/:id` (data) - Send outgoing packet to socket
* `netd/tcp/send/:id` (data) - Send outgoing data to socket

