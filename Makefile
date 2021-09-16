NASM = nasm

bin=build/bin
kern_bin=my_kernel/target/x86_64-my_os/debug

all: $(bin) $(bin)/boot.bin

$(bin):
	mkdir -p $(bin)

$(bin)/boot.bin: $(bin)/boot0.bin $(bin)/boot1.bin kernel
	cat $(bin)/boot0.bin $(bin)/boot1.bin $(bin)/kernel.img > $(bin)/boot.bin

$(bin)/boot0.bin: boot0.asm
	$(NASM) boot0.asm -f bin -o $(bin)/boot0.bin

$(bin)/boot1.bin: boot1.asm e820mem.asm
	$(NASM) boot1.asm -f bin -o $(bin)/boot1.bin

# runs every time since cargo manages source files maybe clean up later
kernel:
	make -C my_kernel
	python3 kernheader.py $(kern_bin)/my_kernel $(bin)/header.bin
	cat $(bin)/header.bin $(kern_bin)/my_kernel > $(bin)/kernel.img

.PHONY : clean
clean:
	make -C my_kernel clean
	rm -rf build $(bin)/boot.bin

run: all
	qemu-system-x86_64 -drive format=raw,file=$(bin)/boot.bin -m size=4096 -d int -M smm=off -no-reboot -monitor stdio

# -monitor stdio

debug: all
	# qemu-system-x86_64 -drive format=raw,file=$(bin)/boot.bin -S -s -m size=4096
	qemu-system-x86_64 -drive format=raw,file=$(bin)/boot.bin -S -s -m size=4096 -d int -M smm=off
