[org 0x7c00] ; where program expects to be placed
; alternatively could do:
; mov ax, 0x07c0
; mov ds, ax
; I think the org method is better as the code
; if mapped to 0x7c00 will make sense

[bits 16] ; run in 16bit mode
boot0_entry:
    cli ; Disable interrupts because no table is setup
    cld ; clear direction flag

    ; ensure cs is 0
    jmp 0:force_cs
force_cs:

    ; print bootloader started message
    mov si, boot_start_msg
    jmp bios_print
bios_print_resume:
    ; Clear All Segment Registers
    ; This is to normalize environment
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov fs, ax
    mov gs, ax

    jmp read_sector

bios_print: ; https://grandidierite.github.io/bios-interrupts/ service 14
    lodsb ; loads byte at si into al, and increments si
    test al, al ; sets 0 reg if al 0, same as or al, al or cmp al, 0
    jz done
    mov ah, 0x0e ; BIOS teletype instruction
    mov bh, 0 ; page number. You can have multiple pages but we just use 0
    int 0x10 ; BIOS interrupt
    jmp bios_print
done:
    jmp bios_print_resume

read_sector:
    mov ah, 2
    mov bx, 0x1000 ; address to place sector
    mov ch, 0 ; cyl 0
    mov dh, 0 ; head 0
              ; boot0 is on sec 0
    mov cl, 2 ; boot 1 is on sec 1 but range is 1-63 (not 0 indexed)
    mov al, 0x10 ; sec to read 0x10 sectors or 8192 bytes
    int 0x13

    cmp al, 0x10
    jne error ; print msg again to signify fail

    mov al, [bx]
    cmp al, 0xbc ; check that first byte is same as what we expect for
                 ; boot1, this may change but can be checked using hexdump
    jne error

    jmp success

error:
    mov si, error_msg
error_print:
    lodsb ; loads byte at si into al, and increments si
    test al, al ; sets 0 reg if al 0, same as or al, al or cmp al, 0
    jz done_error
    mov ah, 0x0e ; BIOS teletype instruction
    mov bh, 0 ; page number. You can have multiple pages but we just use 0
    int 0x10 ; BIOS interrupt
    jmp error_print
done_error:
    hlt

success:
    mov si, suc_msg
suc_print:
    lodsb ; loads byte at si into al, and increments si
    test al, al ; sets 0 reg if al 0, same as or al, al or cmp al, 0
    jz done_suc
    mov ah, 0x0e ; BIOS teletype instruction
    mov bh, 0 ; page number. You can have multiple pages but we just use 0
    int 0x10 ; BIOS interrupt
    jmp suc_print
done_suc:
    jmp 0x1000

    ; Data
boot_start_msg db 'boot0 Started', 13, 10, 0 ; \r\n\0
dw 0x0000
error_msg db 'ERROR: Could not load boot1', 13, 10, 0 ; \r\n\0
dw 0x0000
suc_msg db 'SUCCESS: Loaded boot1 at 0x1000', 13, 10, 0 ; \r\n\0

times 510-($-$$) db 0 ; pad file out with 0s to be proper sector size
dw 0xaa55
    

