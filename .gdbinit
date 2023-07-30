target remote localhost:1234
symbol-file my_kernel/target/x86_64-my_os/release/my_kernel
add-symbol-file my_kernel/target/x86_64-my_os/release/my_kernel -o 0xffff7fffffe00000
# offset is ELF_NEW_BASE - ELF_OLD_BASE
