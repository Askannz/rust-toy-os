#!/usr/bin/env python3

import os
import sys
import shutil
from pathlib import Path
import subprocess

def main():

    #
    # Building WASM apps

    apps_list = [
        "chronometer",
        "cube_3d",
        "terminal",
        "web_browser",
    ]

    for app in apps_list:

        wasm_bin_path = _build_crate(
            crate_path=f"wasm_apps/{app}/",
            binary_name=f"{app}.wasm",
            target="wasm32-wasi"
        )

        _copy_if_new(wasm_bin_path, Path("kernel/wasm") / wasm_bin_path.name)

    #
    # Building kernel

    kernel_bin_path = _build_crate(
        crate_path="kernel/",
        binary_name="kernel.efi",
        target="x86_64-unknown-uefi"
    )

    _copy_if_new(kernel_bin_path, Path("esp/efi/boot/") / "bootx64.efi")

    #
    # Running QEMU

    qemu_args = " ".join([

        "-enable-kvm",
        "-m 4G",
        "-rtc base=utc",
        "-display gtk,zoom-to-fit=off",

        # UEFI boot
        "-drive if=pflash,format=raw,readonly=on,file=uefi_firmware/code.fd",
        "-drive if=pflash,format=raw,readonly=on,file=uefi_firmware/vars.fd",
        "-drive format=raw,file=fat:rw:esp",

        # VirtIO peripherals
        "-device virtio-keyboard",
        "-device virtio-mouse",
        "-device virtio-net-pci,netdev=network0 -netdev user,id=network0",
        "-vga virtio",

        # Debugging
        "-monitor stdio",
        "-serial file:log.txt",
        #"-object filter-dump,id=f1,netdev=network0,file=dump.dat",
    ])

    try:
        subprocess.check_call(f"qemu-system-x86_64 {qemu_args}", shell=True)
    except KeyboardInterrupt:
        sys.exit(1)


def _build_crate(
    crate_path,
    binary_name,
    target,
    mode="release",
):

    crate_path = Path(crate_path)

    binary_path = crate_path / "target" / target / mode / binary_name

    if (
        binary_path.exists() and
        not _check_source_changed(crate_path, binary_path.lstat().st_mtime)
    ):
        print(f"Skipping build for {crate_path} (up-to-date)")
        return binary_path

    print(f"Building {binary_path}")
    try:
        subprocess.check_call(
            f"cargo build --{mode}",
            cwd=crate_path,
            shell=True
        )
    except subprocess.CalledProcessError:
        print("Build failed.")
        sys.exit(1)

    return binary_path


def _check_source_changed(crate_path, binary_mtime):

    files_list = []
    for dirpath, _, filenames in os.walk(crate_path):
        for name in filenames:
            path = Path(dirpath) / name
            if (crate_path / "target") not in path.parents:
                files_list.append(path)

    changed_list = [
        path for path in files_list
        if path.lstat().st_mtime > binary_mtime
    ]

    changed = len(changed_list) > 0

    if changed:
        print(f"Source changes in {crate_path}:")
        print("\n".join(f' - {p}' for p in changed_list))

    return changed


def _copy_if_new(src, dst):
    if (
        not dst.exists() or
        dst.lstat().st_mtime < src.lstat().st_mtime
    ):
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)

if __name__ == "__main__":
    main()
