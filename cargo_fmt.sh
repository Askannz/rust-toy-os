#!/bin/bash

set -e

#
# WASM apps

cd wasm_apps/

cd cube_3d/
cargo fmt
cd ../

cd chronometer/
cargo fmt
cd ../

cd terminal/
cargo fmt
cd ../

cd web_browser/
cargo fmt
cd ../

cd ../

#
# Kernel

cd kernel/
cargo fmt
cd ../

#
# applib

cd applib/
cargo fmt
cd ../

#
# guestlib

cd guestlib/
cargo fmt
cd ../
