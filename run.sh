#!/bin/bash

set -e

#
# Building apps

cd apps/

cd cube_3d/
cargo build --release
cd ../

cd chronometer/
cargo build --release
cd ../

cd ../


#
# Embedding binary data

mkdir -p embedded_data/apps/
python dump_pe.py apps/cube_3d/target/x86_64-unknown-uefi/release/cube_3d.efi embedded_data/apps/cube_3d
python dump_pe.py apps/chronometer/target/x86_64-unknown-uefi/release/chronometer.efi embedded_data/apps/chronometer
python dump_image_bytes.py fontmap.png embedded_data/fontmap.bin
python dump_image_bytes.py wallpaper.png embedded_data/wallpaper.bin


#
# Building kernel

cd kernel/
cargo build --release
cd ../


#
# Running QEMU

mkdir -p esp/efi/boot/
cp kernel/target/x86_64-unknown-uefi/release/kernel.efi esp/efi/boot/bootx64.efi

qemu-system-x86_64 -enable-kvm \
    -m 4G \
    -drive if=pflash,format=raw,readonly=on,file=uefi_firmware/code.fd \
    -drive if=pflash,format=raw,readonly=on,file=uefi_firmware/vars.fd \
    -drive format=raw,file=fat:rw:esp \
    -serial stdio \
    -device virtio-mouse \
    -vga virtio \
    -display gtk,zoom-to-fit=off

    #-device virtio-gpu,xres=1280,yres=720
    #-device virtio-mouse 
    # -vga virtio \
    #-serial stdio
    #-monitor stdio
    #--trace virt*
