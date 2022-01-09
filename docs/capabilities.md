# Capability-based security model

In D7, access control is implemented through capabilities. Both the kernel and some services require the called to provide a capability that proves their right to it. For system calls, the access right is implicitly checked by the kernel, using pid and it's associated security context. If a service needs per-caller permissions, the authentication is done using a capability token.

## Security contexts

For each process, kernel keeps a security context, which is essentially a set of capabilities. An alternative context, which granted to processes created with `exec`, is also provided. The interface somewhat resembles `pledge` syscall from BSD.

A process with empty capability set can only perform the following operations:

* Exit with an exit code
* Read it's own PID
* Read it's own security context
* Yield it's scheduler slice
* Read and write already-shared memory regions

There is following operations available for the main security context (given that the process hasn't given up the associed capabilities):

* Remove or reduce an existing access right
* Gain addition right by redeeming a capability token

For `exec` security context, the operations are:

* Copy current process access rights here
* Remove or reduce an existing access right

## Kernel-checked access rights

Most system calls have an associated capability. These are tracked by the kernel itself. The IPC calls have more specific access controls: both subscriptions and sending are restricted by topic prefixes. This by itself provides enough security for most simple services. For instance, a network card can only be accessed by it's driver. In addition some calls, such as `exit` and memory mapping modification, are only available for the process itself.

## Service-checked access rights

Sometimes having the kernel check the access rights to an IPC prefix isn't fine-grained enough. In these cases the program itself can keep issue capability tokens to callers. The permissions granted by the token can either be encoded into the token itself (if it fits), or kept separately encoding their identifier into the token.

## Capability tokens

An authorization can be transferred across process bounaries by crafting a *capability token* and sending that to the other process. Capability is a cryptographically-signed message that contains the following information:

* Process id that grants the capability (zero for kernel) (u64)
* Capability that is granted (u64)
* Kernel signature (64 bytes, currently ed25519)
