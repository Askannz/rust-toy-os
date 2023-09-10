#!/bin/bash

set -e


cd kernel/
cargo build --release

cd ../
mkdir -p esp/efi/boot/
cp kernel/target/x86_64-unknown-uefi/release/toy-os-kernel.efi esp/efi/boot/bootx64.efi

qemu-system-x86_64 -enable-kvm \
    -m 4G \
    -drive if=pflash,format=raw,readonly=on,file=uefi_firmware/OVMF_CODE.fd \
    -drive if=pflash,format=raw,readonly=on,file=uefi_firmware/OVMF_VARS.fd \
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
