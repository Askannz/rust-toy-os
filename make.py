#!/usr/bin/env python3

import os
import sys
import shutil
import argparse
from pathlib import Path
import subprocess


WASM_APPS = [
    "chronometer",
    "cube_3d",
    "terminal",
    "web_browser",
]

CRATE_PATHS = [
    "kernel/",
    "applib/",
    "guestlib/",
    *[f"wasm_apps/{app}" for app in WASM_APPS]
]


def main():

    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="cmd", required=True)
    subparsers.add_parser("build")
    subparsers.add_parser("run")
    subparsers.add_parser("fmt")
    subparsers.add_parser("fix")
    args = parser.parse_args()

    if args.cmd == "build":
        _build()
    elif args.cmd == "run":
        _build()
        _run()
    elif args.cmd == "fmt":
        _fmt()
    elif args.cmd == "fix":
        _fix()


def _build():

    #
    # Building WASM apps

    for app in WASM_APPS:

        wasm_bin_path = _build_crate(
            crate_path=f"wasm_apps/{app}/",
            binary_name=f"{app}.wasm",
            target="wasm32-wasi",
            dep_paths=["applib/", "guestlib/"],
        )

        _copy_if_new(wasm_bin_path, Path("kernel/wasm") / wasm_bin_path.name)

    #
    # Building kernel

    kernel_bin_path = _build_crate(
        crate_path="kernel/",
        binary_name="kernel.efi",
        target="x86_64-unknown-uefi",
        dep_paths=["applib/"],
    )

    _copy_if_new(kernel_bin_path, Path("esp/efi/boot/") / "bootx64.efi")


def _run():

    #
    # Running QEMU

    qemu_args = " ".join(
        [
            "-enable-kvm",
            "-m 4G",
            "-rtc base=utc",
            "-display sdl",

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
            # "-object filter-dump,id=f1,netdev=network0,file=dump.dat",
        ]
    )

    try:
        subprocess.check_call(f"qemu-system-x86_64 {qemu_args}", shell=True)
    except (KeyboardInterrupt, subprocess.CalledProcessError):
        sys.exit(1)


def _fmt():
    for crate_path in CRATE_PATHS:
        subprocess.check_call("cargo fmt", shell=True, cwd=crate_path)


def _fix():
    for crate_path in CRATE_PATHS:
        subprocess.check_call("cargo fix --allow-dirty", shell=True, cwd=crate_path)


def _build_crate(
    crate_path,
    binary_name,
    target,
    mode="release",
    dep_paths=None,
):

    crate_path = Path(crate_path)

    binary_path = crate_path / "target" / target / mode / binary_name
    if binary_path.exists():

        binary_mtime = binary_path.lstat().st_mtime
        needs_build = _check_source_changed(crate_path, binary_mtime)

        if dep_paths is not None:
            needs_build = needs_build or any(
                _check_source_changed(path, binary_mtime)
                for path in dep_paths
            )

        if not needs_build:
            print(f"Skipping build for {crate_path} (up-to-date)")
            return binary_path

    mode_arg = "" if mode == "debug" else "--release"

    print(f"Building {binary_path}")
    try:
        subprocess.check_call(f"cargo build {mode_arg}", cwd=crate_path, shell=True)
    except (KeyboardInterrupt, subprocess.CalledProcessError):
        print("Build failed.")
        sys.exit(1)

    return binary_path


def _check_source_changed(crate_path, binary_mtime):

    crate_path = Path(crate_path)

    files_list = []
    for dirpath, _, filenames in os.walk(crate_path):
        for name in filenames:
            path = Path(dirpath) / name
            if (crate_path / "target") not in path.parents:
                files_list.append(path)

    changed_list = [path for path in files_list if path.lstat().st_mtime > binary_mtime]

    changed = len(changed_list) > 0

    if changed:
        print(f"Source changes in {crate_path}:")
        print("\n".join(f" - {p}" for p in changed_list))

    return changed


def _copy_if_new(src, dst):
    if not dst.exists() or dst.lstat().st_mtime < src.lstat().st_mtime:
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)


if __name__ == "__main__":
    main()
