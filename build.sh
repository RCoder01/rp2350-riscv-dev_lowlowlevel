#!/usr/bin/sh

set -eu pipefail

cargo +nightly build --release
riscv32-elf-objcopy -I elf32-littleriscv -O binary ./target/riscv32imac-unknown-none-elf/release/riscv-freestanding blink.bin
clif generate -i blink.bin -o blink.uf2 -p 256 -f RP2350_RISCV -t 0x10000000 --fill 0x00

./deploy.sh
