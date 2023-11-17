#!/bin/bash

set -e

#
# Building WASM apps

cd wasm_apps/

cd cube_3d/
cargo build --release
cd ../

cd chronometer/
cargo build --release
cd ../

cd text_editor/
cargo build --release
cd ../

cd ../


#
# Embedding binary data

python dump_image_bytes.py applib/fontmap.png applib/fontmap.bin
python dump_image_bytes.py wallpaper.png embedded_data/wallpaper.bin
cp wasm_apps/cube_3d/target/wasm32-unknown-unknown/release/cube_3d.wasm embedded_data/cube_3d.wasm
cp wasm_apps/chronometer/target/wasm32-unknown-unknown/release/chronometer.wasm embedded_data/chronometer.wasm
cp wasm_apps/text_editor/target/wasm32-unknown-unknown/release/text_editor.wasm embedded_data/text_editor.wasm


#
# Building kernel

cd kernel/
cargo build --release
cd ../


#
# Running QEMU

mkdir -p esp/efi/boot/
cp kernel/target/x86_64-unknown-uefi/release/kernel.efi esp/efi/boot/bootx64.efi

sudo qemu-system-x86_64 -enable-kvm \
    -m 4G \
    -drive if=pflash,format=raw,readonly=on,file=uefi_firmware/code.fd \
    -drive if=pflash,format=raw,readonly=on,file=uefi_firmware/vars.fd \
    -drive format=raw,file=fat:rw:esp \
    -device virtio-keyboard \
    -device virtio-mouse \
    -vga virtio \
    -display gtk,zoom-to-fit=off \
    -device virtio-net-pci,netdev=network0 -netdev tap,id=network0,ifname=tap0,script=no,downscript=no \
    -monitor stdio \
    -serial file:log.txt

    #-nic user,model=virtio-net-pci,hostfwd=tcp::8888-:22 \
    #-nic bridge,br=br0,model=virtio-net-pci \
    #-object filter-dump,id=f1,netdev=network0,file=dump.dat \

    #-device virtio-gpu,xres=1280,yres=720
    #-device virtio-mouse 
    # -vga virtio \
    #-serial stdio
    #-monitor stdio
    #--trace virt*
