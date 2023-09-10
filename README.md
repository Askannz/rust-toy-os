A simple toy OS developed in Rust.

https://github.com/Askannz/rust-toy-os/assets/9202863/e3d5873c-92c6-49ef-9238-2cf9da4bbf94

Features:
* VirtIO drivers for mouse and graphics
* Can load PE executable (kinda, sort of, doesn't support relocation yet)
* Very simple compositing allowing each app to draw to their own framebuffer

### Build and run

See `./run.sh`. Needs QEMU, Rust nightly, Python and a few Python packages (see requirements.txt).

### Resources

* https://os.phil-opp.com/
* https://wiki.osdev.org
* https://docs.oasis-open.org/virtio/virtio/v1.1/csprd01/virtio-v1.1-csprd01.html
* https://github.com/KDE/breeze for the wallpaper
