#!/usr/bin/sh

set -eu pipefail

cargo +nightly build --release
riscv32-elf-objcopy -I elf32-littleriscv -O binary ./target/riscv32imac-unknown-none-elf/release/riscv-freestanding blink
./fix.rs -i blink -o blink.bin
clif generate -i blink.bin -o blink.uf2 -p 256 -f RP2350_RISCV -t 0x10000000

MOUNT_LOCATION=/run/media/$USER/RP2350
sudo mkdir -p $MOUNT_LOCATION || echo "mount location existss"
sudo mount /dev/sda1 $MOUNT_LOCATION
sudo cp ./blink.uf2 $MOUNT_LOCATION 
sudo umount $MOUNT_LOCATION
sudo rm -r $MOUNT_LOCATION
