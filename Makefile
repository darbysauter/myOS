NASM = nasm

bin=build/bin
kern_bin=my_kernel/target/x86_64-my_os/debug
disk_img=fs.img
programs_dir=build/user_programs

all: $(bin) $(bin)/boot.bin $(bin)/$(disk_img)

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

$(bin)/$(disk_img): $(programs_dir)/*
	make -C user_programs
	python3 filesystem_gen.py $(bin)/$(disk_img) $(programs_dir)

.PHONY : clean
clean:
	make -C my_kernel clean
	rm -rf build $(bin)/boot.bin

run: all
	qemu-system-x86_64 -drive format=raw,file=$(bin)/boot.bin -m size=4096 -M smm=off -monitor stdio -d int -drive id=disk,file=$(bin)/$(disk_img),if=none -device ahci,id=ahci -device ide-hd,drive=disk,bus=ahci.0

# -monitor stdio
# -no-reboot

debug: all
	# qemu-system-x86_64 -drive format=raw,file=$(bin)/boot.bin -S -s -m size=4096
	qemu-system-x86_64 -drive format=raw,file=$(bin)/boot.bin -S -s -m size=4096 -d int -M smm=off -monitor stdio
