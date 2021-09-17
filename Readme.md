# Summary

This OS+Bootloader was used to strengthen my knowledge of Rust, x86, and Operating Systems

The bootloader is written fully in assembly and the kernel in Rust

# Building

`$ make` to build

`$ make run` to run in qemu

`$ make debug` to wait for gdb to attach

`$ make clean` to clean build artifacts

# Requirements
Rust (nightly)

nasm

qemu-system-x86_64

python3

# Process

The bootloader will get the processor running in 64 bit mode and identity map all of memory

Once the kernel is loaded it creates a frame allocator using the boot info from the bootloader.

To do this it will compare the good memory regions with pages that overlap.
It skips the region starting at 0x0 and all regions that are not type 0x1.
It will create a singly linked list where the first u64 of the page points to the phys addr of the next page.
The last page will point to 0xdeadbeef just as a safety check.o

Using the frame allocator the heap is created, initially with as much contiguous pages as there is,
and then extended. This is done because I keep track of the contiguous regions of the heap, the first time I don't know how many contiguous memory regions there will be so I just limit this to one. Once the heap is initialized I can now return heap regions in a vector for when I don't know how many regions will be needed for some heap size.

I then create a new page table with a recursive index, map in the heap at a higher and contiguous address, the stack, map the elf at the current mapping, and at some new mapping (high address), I also identity map the vga buffer.

I then translate anything which has a heap address to the new mapping

I then jump to the second phase which will be using the new page table.

The phase 2 function will never return. Also to help with the transition I have a helper function to convert the arguments back to proper safe types and then call into the actual phase 2 function.

On entry of phase 2 the stack and heap should be setup with at high addresses and with contiguous virtual pages, and the kernel mapped into a high address with the frame allocator containing all of the unused frames.

# TODO
interrupts and exceptions

file system
processes and threads

IPC

Video

IO

Networking

# Current state
Trying to figure out how to get an executable that is 100% Position independent, currently there is still GOT, Vtables and string references that have absolute addresses. I could translate these, but it would be cleaner if I did not need to, and I think it is possible: https://github.com/rust-lang/rust/issues/87934#issuecomment-916930145

# References
https://wiki.osdev.org/

https://os.phil-opp.com/

https://github.com/rust-osdev/bootloader

https://github.com/gamozolabs/chocolate_milk

probably others I am missing
