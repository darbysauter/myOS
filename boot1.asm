[org 0x1000] ; where boot1 expects to be placed

[bits 16] ; run in 16bit mode
boot1_entry:
    ; Setup stack
    mov sp, 0x7c00

    ; print boot1 started message
    mov si, boot1_start_msg
    call print_l

    ; Enable A20 Gate "Fast"
    ; Explanation: https://www.win.tue.nl/~aeb/linux/kbd/A20.html
    in al, 0x92 ; read port 0x92
    or al, 2 ; set bit 1 "A20"
    out 0x92, al ; write port 0x92

    ; Store original segments
    push ds
    push es

    ; load global descriptor table
    lgdt [gdt_pointer]

    ; switch to protected mode
    mov eax, cr0
    or al, 1 ; set protected mode bit (0th)
    mov cr0, eax

    jmp protected_mode_entry ; stop crash
protected_mode_entry:
    mov ax, DATA_SEG ; load GDT offset 0x10 into all selectors
    mov ds, ax
    mov es, ax

    ; back to real mode but with segments modified in protected mode
    and al, 0xfe
    mov cr0, eax

    pop es
    pop ds
    sti

    mov si, unreal_enter_msg
    call print_l

set_target_operating_mode:
    ; Some BIOSs assume the processor will only operate in Legacy Mode. We change the Target
    ; Operating Mode to "Long Mode Target Only", so the firmware expects each CPU to enter Long Mode
    ; once and then stay in it. This allows the firmware to enable mode-specifc optimizations.
    ; We save the flags, because CF is set if the callback is not supported (in which case, this is
    ; a NOP)
    pushf
    mov ax, 0xec00
    mov bl, 0x2
    int 0x15
    popf


read_kern_header:
    mov ah, 2
    mov bx, 0x3000 ; address to place sector
    mov ch, 0 ; cyl 0
    mov dh, 0 ; head 0
    mov cl, 0x12 ; sec 2
    mov al, 0x1 ; sec to read 0x10 sectors or 8192 bytes
    int 0x13

    cmp al, 0x1
    jne kern_error ; print msg again to signify fail

    mov eax, [bx]
    cmp eax, 0x22110099 ; check that first byte is same as what we expect for
                 ; boot1, this may change but can be checked using hexdump
    jne kern_error

    call kern_success
    mov bx, 0x3000 ; address to place sector

    add bx, 4
    mov eax, [bx] ; size of kernel
    mov [kern_size], eax
    mov [boot_info_elf_size], eax

    add bx, 4
    mov esi, [bx] ; entry point
    mov [kern_entry], esi

    mov ecx, eax
    add ecx, 511 ; round up
    shr ecx, 9 ; number of blocks (size / 512)

kern_load_init:
    mov ax, 0x3000 ; sector buffer
    mov [dap_buffer_addr], ax

    mov word [dap_blocks], 1 ; 1 block at a time

    mov eax, 0x12
    mov [dap_start_lba], eax ; start sector lba
    ; here is 0 based instead of int0x13 ah->2 which is 1 based
    ; boot0 is on lba 0
    ; boot1 is on lba 0x1-0x10
    ; kernel header is on lba 0x11
    ; kernel is on lba 0x12-(0x12+ecx-1)
    ; hardcoded based off size of our bootloader

    mov edi, 0x4000000 ; where entire kernel will be copied
    mov [boot_info_elf], edi
    ; https://wiki.osdev.org/Memory_Map_(x86)
    ; everything below here is messy (mostly taken)
    ; any is only 1MiB which I am not worried about losing
    ; This should be 14 MiB to 0x00EFFFFF which our kernel
    ; will likely stay smaller than
    ; should look into where Grub and other bootloaders load
    ; kernel

    ; ecx has number of sectors

kern_load_sector:
    mov si, dap
    mov ah, 0x42
    int 0x13
    jc kern_error

    push ecx ; save num of sectors
    push esi ; test if this is necessary
    mov ecx, 512 / 4 ; movsd does 4 bytes so this will be how many operations
    movzx esi, word [dap_buffer_addr] ; zero extended

    rep a32 movsd ; copy from buf to kernel destination esi -> edi
    pop esi ; test if this is necessary
    pop ecx ; restore num of sectors

    ; increment sector to load
    mov eax, [dap_start_lba] ; sector to load
    add eax, 1 ; increment
    mov [dap_start_lba], eax ; sector to load

    ; decrement num sectors to load
    sub ecx, 1
    jnz kern_load_sector

    ; kernel should be loaded at 0x4000000
    mov ebx, 0x4000000
    mov eax, [ebx]
    cmp eax, 0x464c457f ; check that first byte is same as what we expect for
                 ; kernel
                 ;; HERE123

    jne kern_error

    call kern_load_success

get_memory_map:
    call do_e820

    call memory_map_print


    mov ax, 0x3
    int 0x10 ; set vga text mode 3

reenter_protected_mode:
    cli
    lgdt [gdt_pointer]

    mov eax, cr0
    or al, 1    ; set protected mode bit
    mov cr0, eax

    ; push 0x8 ; will be popped into CS
    ; mov eax, protected_mode_reentry ; will be returned to
    ; push eax
    ; retf

    jmp CODE_SEG:protected_mode_reentry ; jump to 32 bit code

kern_error:
    mov si, kern_error_msg
kern_error_print:
    lodsb ; loads byte at si into al, and increments si
    test al, al ; sets 0 reg if al 0, same as or al, al or cmp al, 0
    jz kern_done_error
    mov ah, 0x0e ; BIOS teletype instruction
    mov bh, 0 ; page number. You can have multiple pages but we just use 0
    int 0x10 ; BIOS interrupt
    jmp kern_error_print
kern_done_error:
    cli
    hlt

kern_success:
    mov si, kern_suc_msg
    jmp print_l

kern_load_success:
    mov si, kern_load_suc_msg
    jmp print_l

memory_map_print:
    mov si, mem_map_msg
    jmp print_l

print_l:
    lodsb ; loads byte at si into al, and increments si
    test al, al ; sets 0 reg if al 0, same as or al, al or cmp al, 0
    jz print_loop_done
    mov ah, 0x0e ; BIOS teletype instruction
    mov bh, 0 ; page number. You can have multiple pages but we just use 0
    int 0x10 ; BIOS interrupt
    jmp print_l
print_loop_done:
    ret


[bits 32]
align 4
protected_mode_reentry:
    mov ax, DATA_SEG
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    ; VGA text buff
    mov esi, prot_msg
    call my_vga_print

check_cpu_properties:
    call check_cpuid

    mov esi, checked_cpu_msg
    call my_vga_print

    call check_long_mode

    mov esi, checked_long_msg
    call my_vga_print

    cli ; disable interrupts

    lidt [zero_idt] ; Load a zero length IDT so that any NMI causes a triple fault.

set_up_page_tables:
    ; zero out buffer for page tables
    mov edi, 0x9000 ; pagetable start
    mov ecx, 0x34000 ; pagetable end
    sub ecx, edi ; size of pagetable in bytes
    shr ecx, 2 ; divide by 4 to get dwords
    xor eax, eax ; eax = 0
    rep stosd ; store 4 bytes from eax into address at edi and increment edi

    ; PML4
    mov eax, 0xa000 ; PDPT
    or eax, (1 | 2) ; present and read/write
    mov [0x9000], eax ; PML4[0] -> PDPT moving 32 bit but this is really 64bit
    ; PDPT
    mov eax, 0xb000 ; PD
    or eax, (1 | 2)
    mov [0xa000], eax ; PDPT[0] -> PD
    ; PD
    mov eax, (1 | 2 | (1 << 7)) ; present and read/write and uses 2MB phys pages
    mov ecx, 0
map_p2_table:
    mov [0xb000 + ecx * 8], eax
    add eax, 0x200000 ; maps first 512 * 0x200000 or 1 GiB
    add ecx, 1
    cmp ecx, 512
    jb map_p2_table

enable_paging:
    ; Write back cache and add a memory fence. I'm not sure if this is
    ; necessary, but better be on the safe side.
    wbinvd
    mfence

    ; load PML4 to cr3 register (cpu uses this to access the PML4 table)
    mov eax, 0x9000
    mov cr3, eax

    ; enable PAE-flag in cr4 (Physical Address Extension)
    mov eax, cr4
    or eax, (1 << 5)
    mov cr4, eax

    ; set the long mode bit in the EFER MSR (model specific register)
    mov ecx, 0xC0000080
    rdmsr
    or eax, (1 << 8)
    wrmsr

    ; enable paging in the cr0 register
    mov eax, cr0
    or eax, (1 << 31)
    mov cr0, eax

load_64bit_gdt:
    lgdt [gdt_64_pointer]                ; Load GDT.Pointer defined below.

jump_to_long_mode:
    jmp CODE_SEG_64:long_mode_entry ; jump to longmode

check_cpuid:
    ; Check if CPUID is supported by attempting to flip the ID bit (bit 21)
    ; in the FLAGS register. If we can flip it, CPUID is available.

    ; Copy FLAGS in to EAX via stack
    pushfd
    pop eax

    ; Copy to ECX as well for comparing later on
    mov ecx, eax

    ; Flip the ID bit
    xor eax, (1 << 21)

    ; Copy EAX to FLAGS via the stack
    push eax
    popfd

    ; Copy FLAGS back to EAX (with the flipped bit if CPUID is supported)
    pushfd
    pop eax

    ; Restore FLAGS from the old version stored in ECX (i.e. flipping the
    ; ID bit back if it was ever flipped).
    push ecx
    popfd

    ; Compare EAX and ECX. If they are equal then that means the bit
    ; wasn't flipped, and CPUID isn't supported.
    cmp eax, ecx
    je no_cpuid
    ret
no_cpuid:
    mov esi, no_cpuid_str
    call my_vga_print
no_cpuid_spin:
    hlt
    jmp no_cpuid_spin

check_long_mode:
    ; test if extended processor info in available
    mov eax, 0x80000000    ; implicit argument for cpuid
    cpuid                  ; get highest supported argument
    cmp eax, 0x80000001    ; it needs to be at least 0x80000001
    jb no_long_mode        ; if it's less, the CPU is too old for long mode

    ; use extended info to test if long mode is available
    mov eax, 0x80000001    ; argument for extended processor info
    cpuid                  ; returns various feature bits in ecx and edx
    test edx, (1 << 29)    ; test if the LM-bit is set in the D-register
    jz no_long_mode        ; If it's not set, there is no long mode
    ret
no_long_mode:
    mov esi, no_long_mode_str
    call my_vga_print
no_long_mode_spin:
    hlt
    jmp no_long_mode_spin

my_vga_print:
    mov ebx, 0xb8000
my_vga_print_loop:
    lodsb
    or al, al
    jz my_vga_print_done
    or eax, 0x0100
    mov word [ebx], ax
    add ebx, 2
    jmp my_vga_print_loop
my_vga_print_done:
    ret

[bits 64]

long_mode_entry:
    mov ax, DATA_SEG_64

    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    mov esi, entered_long_mode_str
    call my_vga_print


    ; kernel should be loaded at 0x4000000
    mov ebx, 0x4000000
    mov eax, [ebx]
    cmp eax, 0x464c457f ; check that first byte is same as what we expect for
                 ; kernel
                 ;; HERE 123

    je no_err

    hlt
    hlt

no_err:
    mov rax, 0xdeadbeef ; for some reason this fixes printing

    mov esi, kern_test_succ_str
    call my_vga_print

get_max_phys:
    mov rax, 0x8000
    mov ecx, dword [rax] ; number of entries
    mov rax, 0x8008
    mov rsi, 0
mem_map_iter:
    mov rdx, qword [rax] ; base address
    add rax, 8
    mov rdi, qword [rax] ; length

    add rdx, rdi ; end of region

    add rax, 8
    mov edi, dword [rax] ; type
    add rax, 8

    cmp edi, 1  ; check if type is 1, this is a bad workaround because qemu update now adds a huge restricted
                ; region, and mapping such a big region will mean our pagetable gets too big and goes into restricted area
    jne not_greater ; skip if type is not 1

    cmp rdx, rsi

    jle not_greater

    mov rsi, rdx

not_greater:

    sub ecx, 1
    cmp ecx, 0
    jne mem_map_iter

    mov rax, rsi ; this has max phys
    push rax

imap_rest: ; ident map rest of mem for bootloader
    ; get num of 2MiB units (will lose up to < 2MiB max)

    mov rsi, rax
    shr rsi, 21 ; divide by 0x200000 (2^21) to get 2MiB units
    sub rsi, 0x200 ; already done first 1GiB

    mov rax, 0xc000 ; last level table
    or rax, (1 | 2)

    mov rdx, 1 ; how many GiB

    mov rdi, 0x40000000 ; end of 1GiB
    or rdi, (1 | 2 | (1 << 7))
next_pdpt:
    ; PDPT
    mov [0xa000 + rdx * 8], rax ; PDPT[rdx] -> PD
    and al, 0xfc ;; clear bits 1 and 2

    mov rcx, 0
map_pd:
    mov [rax + rcx * 8], rdi ; PD[rcx] -> rax
    add rdi, 0x200000 ; maps first 512 * 0x200000 or 1 GiB
    sub rsi, 1
    cmp rsi, 0
    je done_map_rest

    add rcx, 1
    cmp rcx, 512
    jb map_pd

    add rdx, 1 ; do next GiB
    add rax, 0x1000 ; next PD
    or rax, (1 | 2)

    jmp next_pdpt

done_map_rest: ; all of physical memory is identity mapped for bootloader

kern_first_GiB:
set_up_page_tables_kern:
    ; zero out buffer for page tables
    mov edi, 0x34000 ; pagetable start
    mov ecx, 0x70000 ; pagetable end
    sub ecx, edi ; size of pagetable in bytes
    shr ecx, 2 ; divide by 4 to get dwords
    xor eax, eax ; eax = 0
    rep stosd ; store 4 bytes from eax into address at edi and increment edi

    ; PML4
    mov eax, 0x35000 ; PDPT
    or eax, (1 | 2) ; present and read/write
    mov [0x34000], eax ; PML4[0] -> PDPT moving 32 bit but this is really 64bit
    ; PDPT
    mov eax, 0x36000 ; PD
    or eax, (1 | 2)
    mov [0x35000], eax ; PDPT[0] -> PD
    ; PD
    mov eax, (1 | 2 | (1 << 7)) ; present and read/write and uses 2MB phys pages
    mov ecx, 0
map_p2_table_kern:
    mov [0x36000 + ecx * 8], eax
    add eax, 0x200000 ; maps first 512 * 0x200000 or 1 GiB
    add ecx, 1
    cmp ecx, 512
    jb map_p2_table_kern

imap_rest_kern: ; ident map rest of mem for bootloader
    ; get num of 2MiB units (will lose up to < 2MiB max)

    pop rax ; get max phys
    mov rsi, rax
    shr rsi, 21 ; divide by 0x200000 (2^21) to get 2MiB units
    sub rsi, 0x200 ; already done first 1GiB

    mov rax, 0x37000 ; last level table
    or rax, (1 | 2)

    mov rdx, 1 ; how many GiB

    mov rdi, 0x40000000 ; end of 1GiB
    or rdi, (1 | 2 | (1 << 7))
next_pdpt_kern:
    ; PDPT
    mov [0x35000 + rdx * 8], rax ; PDPT[rdx] -> PD
    and al, 0xfc ;; clear bits 1 and 2

    mov rcx, 0
map_pd_kern:
    mov [rax + rcx * 8], rdi ; PD[rcx] -> rax
    add rdi, 0x200000 ; maps first 512 * 0x200000 or 1 GiB
    sub rsi, 1
    cmp rsi, 0
    je done_map_rest_kern

    add rcx, 1
    cmp rcx, 512
    jb map_pd_kern

    add rdx, 1 ; do next GiB
    add rax, 0x1000 ; next PD
    or rax, (1 | 2)

    jmp next_pdpt_kern

done_map_rest_kern: ; all of physical memory is identity mapped for kernel

    mov rax, 0xdeadbeef ; for some reason this fixes printing
    mov esi, done_page_tables_str
    call my_vga_print

load_elf: ; elf needs to be properly loaded in memory
    mov ebx, 0x4000000
    mov eax, [ebx]
    cmp eax, 0x464c457f ; check that first byte is same as what we expect for

    jne error_64

    ; phdr table
    mov rax, [rbx + 0x20] ; phoff
    movzx rdi, byte [rbx + 0x36] ; phentsize
    movzx rsi, byte [rbx + 0x38] ; phnum
    mov rbx, 0x4000000
    add rbx, rax ; address of phdr table

    mov rcx, 0
load_segment:
    cmp rcx, rsi
    je done_loading_segments

    push rdi ; push ent size
    push rsi ; push num of ents

    mov edx, [rbx] ; type
    cmp rdx, 1 ; check if PT_LOAD
    jne go_next_ent

pt_load_seg:
    mov edx, [rbx + 4] ; flags ; for now we ignore
    mov rdx, [rbx + 8] ; offset in file
    add rdx, 0x4000000
    mov rax, rdx
    and rax, 0xfffffffffffff000 ; round down to nearest 0x1000
    sub rdx, rax ; get difference between where we will start copying
                 ; and where segment lays

    mov rdi, [rbx + 0x20] ; file size
    mov rsi, [rbx + 0x28] ; mem size

    add rdi, rdx ; file size from allignment
    add rsi, rdx ; mem size from allignment

    mov rdx, [rbx + 0x10] ; vaddr
    and rdx, 0xfffffffffffff000 ; round down to nearest 0x1000

    ; at this point we have
    ; rax - source (aligned 0x1000)
    ; rbx - addr of ent which we dont need during copy
    ; rcx - ent index which we dont need during copy
    ; rdx - dest (aligned 0x1000)
    ; rdi - size to copy from file
    ; rsi - size to fill (extra filled with 0s)

    ; start copying
    push rbx ; push addr of ent
    push rcx ; push index of ent

    ; copy byte
copy_elf_byte_loop:
    cmp rsi, 0 ; check if we need to copy any bytes
    je done_copy_seg
    sub rsi, 1

    mov bl, 0
    cmp rdi, 0 ; check if byte should be 0
    je copy_elf_byte

    ; otherwise grab byte from source
    sub rdi, 1 ; decrement bytes to grab
    mov bl, [rax]

copy_elf_byte:
    mov [rdx], bl
    add rax, 1
    add rdx, 1
    jmp copy_elf_byte_loop

done_copy_seg:
    pop rcx ; pop index of ent
    pop rbx ; pop addr of ent
    ; done copying

go_next_ent:
    pop rsi ; pop num of ents
    pop rdi ; pop ent size
    add rcx, 1
    add rbx, rdi ; get next ent
    jmp load_segment


done_loading_segments:

    mov rax, 0xdeadbeef ; for some reason this fixes printing
    mov esi, done_loading_seg_str
    call my_vga_print


handle_relocations: ; do relocations after other segments copied
    mov ebx, 0x4000000

    ; phdr table
    mov rax, [rbx + 0x20] ; phoff
    movzx rdi, byte [rbx + 0x36] ; phentsize
    movzx rsi, byte [rbx + 0x38] ; phnum
    mov rbx, 0x4000000
    add rbx, rax ; address of phdr table

    mov rcx, 0
load_segment_relocation:
    cmp rcx, rsi
    je error_64

    push rdi ; push ent size
    push rsi ; push num of ents

    mov edx, [rbx] ; type
    cmp rdx, 2 ; check if PT_DYNAMIC
    jne go_next_ent_relocations

    mov rdx, [rbx + 8] ; offset in file
    add rdx, 0x4000000 ; offset in file but where we have it loaded

    ; These offsets might change, so we could check type
    mov rax, [rdx + 0x28] ; addr of ents
    mov rsi, [rdx + 0x48] ; ent size
    mov rcx, [rdx + 0x58] ; ent count

relocation_loop:
    cmp rcx, 0
    je done_relocations

    ; do stuff
    mov rdx, [rax] ; address to apply value
    mov rdi, [rax + 0x10] ; value to apply
    mov [rdx], rdi

    add rax, rsi
    sub rcx, 1
    jmp relocation_loop


go_next_ent_relocations:
    pop rsi ; pop num of ents
    pop rdi ; pop ent size
    add rcx, 1
    add rbx, rdi ; get next ent
    jmp load_segment_relocation

done_relocations:

init_bss:
    mov ebx, 0x4000000
    mov rax, [rbx + 0x28] ; shoff
    add rax, 0x4000000
    mov rcx, rax
    add rax, 0x350 ; addr
    add rcx, 0x360 ; size

    mov rax, [rax]
    mov rcx, [rcx]

    mov bl, 0
zero_bss_loop:
    cmp rcx, 0 ; check if we need to copy any bytes
    je done_bss
    sub rcx, 1
    mov [rax], bl
    add rax, 1
    jmp zero_bss_loop

done_bss:

context_switch:
    mov rcx, 0x34000
    mov cr3, rcx
    mov rsp, 0x8000
    mov qword [boot_info_stack], 0x8000
    push 0

    ; fill bootinfo
    mov eax, [0x8000] ; memory map entry number
    mov [boot_info_mm_num_entries], eax
    mov qword [boot_info_mm], 0x8008

    ; arguments to _start
    ; sysv64 calling convention
    ; RDI, RSI, RDX, RCX, R8, R9
    mov rdi, boot_info
    mov rsi, boot_info

    mov ebx, dword [kern_entry] ; zero extended
    jmp rbx ; jump to _start

    hlt
    hlt


error_64:
    hlt
    mov rax, 0xdeadbeef


[bits 16]

%include "e820mem.asm"

; Data
boot1_start_msg db 'Boot1 Started', 13, 10, 0 ; \r\n\0
dw 0x0000
unreal_enter_msg db 'Entered Huge Unreal', 13, 10, 0 ; \r\n\0
dw 0x0000
kern_error_msg db 'ERROR: Could not load kern', 13, 10, 0 ; \r\n\0
dw 0x0000
kern_suc_msg db 'SUCCESS: Loaded kern header at 0x3000', 13, 10, 0 ; \r\n\0
dw 0x0000
kern_load_suc_msg db 'SUCCESS: Loaded kern at 0x4000000', 13, 10, 0 ; \r\n\0
dw 0x0000
mem_map_msg db 'Created Memory Map', 13, 10, 0 ; \r\n\0
dw 0x0000
prot_msg db 'Re-Entered Protected Mode----', 13, 10, 0 ; \r\n\0
dw 0x0000
checked_cpu_msg db 'Checked CPUID------------', 13, 10, 0 ; \r\n\0
dw 0x0000
checked_long_msg db 'Checked Long------------', 13, 10, 0 ; \r\n\0
dw 0x0000
no_cpuid_str db 'Error: CPU does not support CPUID', 13, 10, 0 ; \r\n\0
dw 0x0000
no_long_mode_str db 'Error: CPU does not support long mode', 13, 10, 0 ; \r\n\0
dw 0x0000
entered_long_mode_str db 'Entered long mode!!!!---------', 13, 10, 0 ; \r\n\0
dw 0x0000
kern_test_succ_str db 'Kernel still there!!!---------', 13, 10, 0 ; \r\n\0
dw 0x0000
done_page_tables_str db 'Done Page Tables!!!----------', 13, 10, 0 ; \r\n\0
dw 0x0000
done_loading_seg_str db 'Done Loading Segments!!!----------', 13, 10, 0 ; \r\n\0
dw 0x0000

dap: ; struct for int13h AH->0x42
    db 0x10 ; size
    db 0 ; unused
dap_blocks:
    dw 0 ; num sectors
dap_buffer_addr:
    dw 0 ; buf
dap_buffer_seg:
    dw 0 ; segment of buf
dap_start_lba:
    times 8 db 0 ; start block using lba

kern_entry:
    dd 0
kern_size:
    dd 0

align 8
boot_info:
boot_info_mm_num_entries:
    dd 0 ; mem map num entries
boot_info_mm:
    dq 0 ; mem map address
boot_info_elf:
    dq 0 ; elf address
boot_info_elf_size:
    dd 0 ; elf size
boot_info_stack:
    dq 0 ; top of stack

align 4
zero_idt:
    dw 0
    db 0


align 8
gdt_start:
    dq 0x0
gdt_code:
    dw 0xFFFF
    dw 0x0
    db 0x0
    db 10011010b
    db 11001111b
    db 0x0
gdt_data:
    dw 0xFFFF
    dw 0x0
    db 0x0
    db 10010010b
    db 11001111b
    db 0x0
gdt_end:

align 8
gdt_pointer:
    dw gdt_end - gdt_start
    dd gdt_start
CODE_SEG equ gdt_code - gdt_start
DATA_SEG equ gdt_data - gdt_start

align 8
gdt_64:
    dq 0x0000000000000000          ; Null Descriptor - should be present.
gdt_code_64:
    dq 0x00209A0000000000          ; 64-bit code descriptor (exec/read).
gdt_data_64:
    dq 0x0000920000000000          ; 64-bit data descriptor (read/write).

align 8
gdt_64_pointer:
    dw gdt_64_pointer - gdt_64    ; 16-bit Size (Limit) of GDT.
    dd gdt_64                     ; 32-bit Base Address of GDT. (CPU will zero extend to 64-bit)

CODE_SEG_64 equ gdt_code_64 - gdt_64
DATA_SEG_64 equ gdt_data_64 - gdt_64

times 8192-($-$$) db 0 ; pad file out with 0s 16 * sector size
