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

cd terminal/
cargo build --release
cd ../

cd web_browser/
cargo build --release
cd ../

cd ../


#
# Embedding binary data

mkdir -p embedded_data/
cp wasm_apps/cube_3d/target/wasm32-wasi/release/cube_3d.wasm embedded_data/cube_3d.wasm
cp wasm_apps/chronometer/target/wasm32-wasi/release/chronometer.wasm embedded_data/chronometer.wasm
cp wasm_apps/terminal/target/wasm32-wasi/release/terminal.wasm embedded_data/terminal.wasm
cp wasm_apps/web_browser/target/wasm32-wasi/release/web_browser.wasm embedded_data/web_browser.wasm


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
    -device virtio-net-pci,netdev=network0 -netdev user,id=network0 \
    -object filter-dump,id=f1,netdev=network0,file=dump.dat \
    -monitor stdio \
    -rtc base=utc \
    -serial file:log.txt
    
    # -device virtio-net-pci,netdev=network0 -netdev tap,id=network0,ifname=tap0,script=no,downscript=no \
    # -device virtio-net-pci,netdev=network0 -netdev user,id=network0 \

    #-nic user,model=virtio-net-pci,hostfwd=tcp::8888-:22 \
    #-nic bridge,br=br0,model=virtio-net-pci \
    #-object filter-dump,id=f1,netdev=network0,file=dump.dat \

    #-device virtio-gpu,xres=1280,yres=720
    #-device virtio-mouse 
    # -vga virtio \
    #-serial stdio
    #-monitor stdio
    #--trace virt*
