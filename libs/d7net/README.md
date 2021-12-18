# `d7net` - Network stack data formats

Supported protocols: Ethernet, ARP, IPv4, TCP
Coming soon: UDP, IPv6, DHCP, DNS


## Current limitations

* Error handling: Invalid data always panics

## Unsupported by design

* ARP only supports MAC addresses as HW addresses
* IPv4 Options fields are not supported
* Only the most commonly used TCP Options fields are supported
* TCP Urgency fields are not supported