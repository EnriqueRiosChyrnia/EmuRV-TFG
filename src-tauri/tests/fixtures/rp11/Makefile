# Regeneración de la fixture RP11 (ELF real de gcc).
# Requiere la toolchain de brew: brew install riscv64-elf-gcc
# Los flags son los canónicos de EmuRV (ver README §Abrir binarios).
fixture.elf: fixture.c
	riscv64-elf-gcc -march=rv32im -mabi=ilp32 \
	  -Ttext=0x00400000 -Tdata=0x10010000 -nostdlib -o $@ $<
