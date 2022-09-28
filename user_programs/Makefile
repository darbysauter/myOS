user_programs=../build/user_programs
output=target/debug

build_kernel:
	cargo fmt
	cargo build
	mkdir -p $(user_programs)
	find $(output) -maxdepth 1 -type f -perm -0111 -exec cp {} $(user_programs) \;

.PHONY : clean

clean:
	cargo clean
