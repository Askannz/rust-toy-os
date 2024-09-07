#!/bin/bash

set -e

#
# WASM apps

cd wasm_apps/

cd cube_3d/
cargo fix --allow-dirty
cd ../

cd chronometer/
cargo fix --allow-dirty
cd ../

cd terminal/
cargo fix --allow-dirty
cd ../

cd web_browser/
cargo fix --allow-dirty
cd ../

cd ../

#
# Kernel

cd kernel/
cargo fix --allow-dirty
cd ../

#
# applib

cd applib/
cargo fix --allow-dirty
cd ../

#
# guestlib

cd guestlib/
cargo fix --allow-dirty
cd ../
