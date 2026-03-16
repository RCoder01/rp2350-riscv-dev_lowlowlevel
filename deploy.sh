#!/usr/bin/bash

set -eu pipefail

MOUNT_LOCATION=/run/media/$USER/RP2350
sudo mkdir -p $MOUNT_LOCATION || echo "mount location existss"
sudo mount /dev/sda1 $MOUNT_LOCATION
sudo cp ./blink.uf2 $MOUNT_LOCATION 
sudo umount $MOUNT_LOCATION
sudo rm -r $MOUNT_LOCATION
