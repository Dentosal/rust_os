System call reference
=====================

# Groups

Numbers| Description
-------|-----------------
0x0_   | Misc essentials/utilities for the current process calls
0x3_   | Process control
0x4_   | Misc kernel-provided services
0x5_   | Scheduler
0x6_   | Capabilities
0x7_   | IPC
0x8_   | Misc driver-kernel interfaces
0x9_   | Memory block control


# List

Number | Name              | Arguments (logical)   | On success  | Short description
-------|-------------------|-----------------------|-------------|-------------------
0x00   | exit              | status_code           | !           | Terminate the calling process
0x01   | get_pid           | -                     | pid         | Get pid of the calling process
0x02   | debug_print       | **string**            | -           | Print a UTF-8 string to the kernel terminal
0x30   | exec              | **image**, **args**   | pid         | Execute a file from an elf image
0x40   | random            | seeddata              | random      | Read and seed rng
0x50   | sched_yield       | -                     | -           | Yield control to schedule next process
0x51   | sched_sleep_ns    | ns                    | -           | Sleep specified number of nanoseconds
0x60   | cap_verify        | **buf**               | -           | Verifies a capability token
0x61   | cap_sign          | **buf**, CapId        | -           | Signs a new user-given capability token
0x62   | cap_export        | **buf**               | -           | Signs the current kernel security ctx
0x63   | cap_import        | **buf**               | -           | Adds token-permissions to kernel security ctx
0x64   | cap_reduce        | KCapId, **args**      | -           | Gives up some kernel security capabilities
0x65   | cap_exec_reduce   | KCapId, **args**      | -           | Same as above, but for `exec` capabilities
0x66   | cap_exec_clone    | **buf**               | -           | Copies current caps to `exec` capabilities
0x70   | ipc_subscribe     | **f**, flags          | -           | Subscribes to messages by filter **f**
0x71   | ipc_unsubscribe   | SubId                 | -           | Unsubscribes from messages
0x72   | ipc_publish       | **topic**, **data**   | -           | Publish unreliable message (nonblocking)
0x73   | ipc_deliver       | **topic**, **data**   | -           | Deliver reliable message (blocking)
0x74   | ipc_deliver_reply | **topic**, **data**   | -           | Reply to a reliable message before ack
0x75   | ipc_acknowledge   | SubId,AckId,ok?       | -           | Acknowledge a reliable message
0x76   | ipc_receive       | SubId, **buf**        | byte_count  | Receive a message to **buf** (blocking)
0x77   | ipc_select        | **SubIds**, noblock?  | index       | Wait until first message is available
0x80   | kernel_log_read   | **buffer**            | byte_count  | Read all new logs to **buf** (nonblocking)
0x84   | irq_set_handler   | irq_number, **code**  | -           | Assignes **code** to be ran on irq
0x90   | mmap_physical     | len,paddr,vaddr,flags | *ptr*       | Map phys memory location to process memory
0x92   | dma_allocate      | len                   | PhysAddr    | Allocate DMA-accessible physical memory
0x93   | dma_free          | PhysAddr, len         | -           | Deallocate DMA-accessible physical memory
0x94   | mem_alloc         | **area**, flags       | -           | Create virtual region backed by actual memory
0x95   | mem_dealloc       | **area**              | -           | Free allocated memory
0x96   | mem_share         | **area**, flags       | CapToken    | Create a capability to share memory with

*Cursived* text implies that something is a pointer.
**Bold** text implies that something is a read-only slice, i.e. `len, ptr` pair.
Values like `ok?` ending with `?` represent booleans.

# Call structure

Register | Description
---------|-------------
rax      | Routine number
rdi      | Argument 1
rsi      | Argument 2
rdx      | Argument 3
rcx      | Argument 4

## Return structure

Register | Description
---------|-------------
rax      | Success? Boolean
rdi      | Return value
