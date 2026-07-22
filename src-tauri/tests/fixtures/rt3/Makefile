# Regeneración de la fixture del runner RISCOF (RT-3):
#   make -C src-tauri/tests/fixtures/rt3
sig_fixture.elf: sig_fixture.S
	riscv64-elf-gcc -march=rv32i_zicsr -mabi=ilp32 -static -mcmodel=medany \
	  -nostdlib -nostartfiles -T ../../../../tests-riscv/env-emurv/link.ld \
	  $< -o $@
