cargo build --release
riscv32-elf-objcopy -I elf32-littleriscv -O binary ./target/riscv32imac-unknown-none-elf/release/riscv-freestanding blink
./fix.rs -i blink -o blink.bin
clif generate -i blink.bin -o blink.uf2 -p 256 -f RP2350_RISCV -t 0x10000000

