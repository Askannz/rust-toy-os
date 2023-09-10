A simple toy OS developed in Rust.

https://github.com/Askannz/rust-toy-os/assets/9202863/724ac131-2657-4051-b17a-509c03f8f619

Features:
* VirtIO drivers for mouse and graphics
* Can load PE executable (kinda, sort of, doesn't support relocation yet)
* Very simple compositing allowing each app to draw to their own framebuffer

### Build and run

See `./run.sh`. Needs QEMU, Rust nightly, Python and a few Python packages (see requirements.txt).

### Resources

* https://os.phil-opp.com/
* https://docs.oasis-open.org/virtio/virtio/v1.1/csprd01/virtio-v1.1-csprd01.html
* https://github.com/KDE/breeze for the wallpaper
