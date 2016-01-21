[BITS 64]

global start
extern rust_main

section .entry
start:
    call rust_main
