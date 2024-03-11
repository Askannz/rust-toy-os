use core::mem::size_of;
use core::cell::RefCell;
use alloc::vec;
use alloc::rc::Rc;
use alloc::{string::String, borrow::ToOwned};
use smoltcp::iface::SocketHandle;
use smoltcp::socket::tcp::Socket;
use smoltcp::wire::{EthernetAddress, IpCidr, IpAddress, Ipv4Address};
use wasmi::{Engine, Store, Func, Caller, Module, Linker, Config, TypedFunc, AsContextMut, Instance, AsContext};
use spin::Mutex;

use applib::{SystemState, Framebuffer, Rect};

use crate::network::TcpStack;

pub struct WasmEngine {
    engine: Engine,
}

impl WasmEngine {

    pub fn new() -> Self {
        let engine = Engine::new(&Config::default().consume_fuel(false));
        WasmEngine { engine }
    }

    pub fn instantiate_app(&self, tcp_stack: Rc<RefCell<TcpStack>>, wasm_code: &[u8], app_name: &str, init_rect: &Rect) -> WasmApp {

        let module = Module::new(&self.engine, wasm_code).unwrap();
        let store_data = StoreData::new(tcp_stack, app_name, init_rect);
        let mut store: Store<StoreData> = Store::new(&self.engine, store_data);

        //
        // WASM<->System API functions
    
        let host_print_console = Func::wrap(&mut store, |caller: Caller<StoreData>, addr: i32, len: i32| {
    
            let ctx = caller.as_context();
            let app_name = &ctx.data().app_name;

            let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
            let mem_data = mem.data(&caller);
            let len = len as usize;
            let addr = addr as usize;
            let s = core::str::from_utf8(&mem_data[addr..addr+len]).unwrap().trim_end();

            log::debug!("{}: {}", app_name, s);
        });

        let host_get_system_state = Func::wrap(&mut store, |caller: Caller<StoreData>, addr: i32| {
            let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
            let system_state = caller.data()
                .system_state
                .as_ref().expect("System state not available");
            unsafe {
                let len = size_of::<SystemState>();
                let ptr = system_state as *const SystemState as *const u8;
                let mem_slice = core::slice::from_raw_parts(ptr, len);
                mem.write(caller, addr as usize, mem_slice).unwrap();
            }
        });

        let host_get_win_rect = Func::wrap(&mut store, |caller: Caller<StoreData>, addr: i32| {
            let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
            let win_rect = &caller.data().win_rect;

            unsafe {
                let len = size_of::<Rect>();
                let ptr = win_rect as *const Rect as *const u8;
                let mem_slice = core::slice::from_raw_parts(ptr, len);
                mem.write(caller, addr as usize, mem_slice).unwrap();
            }
        });

        let host_set_framebuffer = Func::wrap(&mut store, |mut caller: Caller<StoreData>, addr: i32, w: i32, h: i32| {
            caller.data_mut().framebuffer = Some(WasmFramebufferDef { 
                addr: addr as usize,
                w: w as u32,
                h: h as u32,
            });
        });

        let host_tcp_connect = Func::wrap(&mut store, |mut caller: Caller<StoreData>, ip_addr: i32, port: i32| {
            let data_mut = caller.data_mut();
            let mut tcp_stack = data_mut.tcp_stack.borrow_mut();

            let ip_bytes = ip_addr.to_le_bytes();
            let port: u16 = port.try_into().expect("Invalid port value");

            let socket_handle = tcp_stack.connect(Ipv4Address(ip_bytes), port);
            data_mut.socket_handle = Some(socket_handle);
        });

        let host_tcp_may_send = Func::wrap(&mut store, |caller: Caller<StoreData>| -> i32 {
            let data = caller.data();
            let tcp_stack = data.tcp_stack.borrow_mut();
            let socket_handle = data.socket_handle.expect("No TCP connection");
            tcp_stack.may_send(socket_handle).into()
        });

        let host_tcp_may_recv = Func::wrap(&mut store, |caller: Caller<StoreData>| -> i32 {
            let data = caller.data();
            let tcp_stack = data.tcp_stack.borrow_mut();
            let socket_handle = data.socket_handle.expect("No TCP connection");
            tcp_stack.may_recv(socket_handle).into()
        });

        let host_tcp_write = Func::wrap(&mut store, |mut caller: Caller<StoreData>, addr: i32, len: i32| -> i32 {

            let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
            let mem_data = mem.data(&caller);
            let len = len as usize;
            let addr = addr as usize;
            let buf = mem_data[addr..addr+len].to_vec();

            let data_mut = caller.data_mut();
            let mut tcp_stack = data_mut.tcp_stack.borrow_mut();

            let socket_handle = data_mut.socket_handle.expect("No TCP connection");

            let written_len = tcp_stack.write(socket_handle, &buf);

            let written_len: i32 = written_len.try_into().unwrap();

            written_len
        });

        let host_tcp_read = Func::wrap(&mut store, |mut caller: Caller<StoreData>, addr: i32, len: i32| -> i32 {

            let len = len as usize;
            let addr = addr as usize;

            let mut buf = vec![0; len];

            let read_len: usize = {
                let data_mut = caller.data_mut();
                let mut tcp_stack = data_mut.tcp_stack.borrow_mut();
                let socket_handle = data_mut.socket_handle.expect("No TCP connection");
                tcp_stack.read(socket_handle, &mut buf)
            };

            let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
            let mem_data = mem.data_mut(&mut caller);

            mem_data[addr..addr+read_len].copy_from_slice(&buf[..read_len]);

            let read_len: i32 = read_len.try_into().unwrap();

            read_len
        });

        //
        // Instantiating app

        let mut linker = <Linker<StoreData>>::new(&self.engine);
        linker.define("env", "host_print_console", host_print_console).unwrap();
        linker.define("env", "host_get_system_state", host_get_system_state).unwrap();
        linker.define("env", "host_get_win_rect", host_get_win_rect).unwrap();
        linker.define("env", "host_set_framebuffer", host_set_framebuffer).unwrap();
        linker.define("env", "host_tcp_connect", host_tcp_connect).unwrap();
        linker.define("env", "host_tcp_may_send", host_tcp_may_send).unwrap();
        linker.define("env", "host_tcp_may_recv", host_tcp_may_recv).unwrap();
        linker.define("env", "host_tcp_write", host_tcp_write).unwrap();
        linker.define("env", "host_tcp_read", host_tcp_read).unwrap();

        add_wasi_functions(&mut store, &mut linker);

        let instance = linker
            .instantiate(&mut store, &module).unwrap()
            .start(&mut store).unwrap();
    
        let wasm_init = instance.get_typed_func::<(), ()>(&store, "init").unwrap();
        let wasm_step = instance.get_typed_func::<(), ()>(&store, "step").unwrap();


        //
        // App init

        log::info!("Initializing {}", app_name);
        wasm_init
            .call(&mut store, ())
            .expect("Failed to initialize WASM app");

        WasmApp { 
            store,
            instance,
            wasm_step,
        }
    }
}

#[derive(Clone)]
struct WasmFramebufferDef {
    addr: usize,
    h: u32,
    w: u32,
}

struct StoreData {
    app_name: String,
    system_state: Option<SystemState>,
    win_rect: Rect,
    framebuffer: Option<WasmFramebufferDef>,

    tcp_stack: Rc<RefCell<TcpStack>>,
    socket_handle: Option<SocketHandle>
}

impl StoreData {
    fn new(tcp_stack: Rc<RefCell<TcpStack>>, app_name: &str, init_rect: &Rect) -> Self {
        StoreData { 
            app_name: app_name.to_owned(),
            system_state: None,
            framebuffer: None,
            win_rect: init_rect.clone(),
            tcp_stack,
            socket_handle: None,
        }
    }
}

pub struct WasmApp {
    store: Store<StoreData>,
    instance: Instance,
    wasm_step: TypedFunc<(), ()>,
}

impl WasmApp {
    pub fn step(&mut self, system_state: &SystemState, system_fb: &mut Framebuffer, win_rect: &Rect) {

        let mut ctx = self.store.as_context_mut();

        ctx.data_mut().system_state = Some(system_state.clone());
        ctx.data_mut().win_rect = win_rect.clone();

        self.wasm_step
            .call(&mut self.store, ())
            .expect("Failed to step WASM app");


        let wasm_fb_def = self.store.as_context().data().framebuffer.clone();
        if let Some(wasm_fb_def) = wasm_fb_def {

            let mem = self.instance.get_memory(&self.store, "memory").unwrap();
            let ctx = self.store.as_context_mut();
            let mem_data = mem.data_mut(ctx);

            let wasm_fb = {
                let WasmFramebufferDef { addr, w, h } = wasm_fb_def;
                let fb_data = &mut mem_data[addr..addr + (w*h*4) as usize];
                let fb_data = unsafe { fb_data.align_to_mut::<u32>().1 };
                Framebuffer::new(fb_data, w, h)
            };

            system_fb.copy_fb(&wasm_fb, &win_rect, false);
        }
    }
}


//
// WASI

fn add_wasi_functions(mut store: &mut Store<StoreData>, linker: &mut Linker<StoreData>) {

    macro_rules! linker_impl {
        ($module:expr, $name:expr, $func:expr) => {
            linker.define(
                $module, $name,
                Func::wrap(&mut store, $func)
            ).unwrap();
        }
    }

    macro_rules! linker_stub {

        ($module:expr, $name:expr, [$($x:ty),*], $y:ty) => {
            linker_impl!(
                $module, $name,
                |_: Caller<StoreData>, $(_: $x),*| -> $y { 
                    panic!("WASM function {}() is not implemented (stub)", $name);
                }
            )
        };

        ($module:expr, $name:expr, [$($x:ty),*], $y:ty, $v:expr) => {
            linker_impl!(
                $module, $name,
                |_: Caller<StoreData>, $(_: $x),*| -> $y {
                    log::debug!("WASM stub {}() called, returning {:?}", $name, $v);
                    $v
                }
            )
        }
    }

    linker_stub!("__main_argc_argv", "env", [i32, i32], i32);

    let m = "wasi_snapshot_preview1";

    linker_stub!(m, "clock_time_get", [i32, i64, i32], i32);
    linker_stub!(m, "fd_filestat_set_size", [i32, i64], i32);
    linker_stub!(m, "fd_read", [i32, i32, i32, i32], i32);
    linker_stub!(m, "fd_readdir", [i32, i32, i32, i64, i32], i32);
    linker_stub!(m, "path_create_directory", [i32, i32, i32], i32);
    linker_stub!(m, "path_filestat_get", [i32, i32, i32, i32, i32], i32);
    linker_stub!(m, "path_link", [i32, i32, i32, i32, i32, i32, i32], i32);
    linker_stub!(m, "path_open", [i32, i32, i32, i32, i32, i64, i64, i32, i32], i32);
    linker_stub!(m, "path_readlink", [i32, i32, i32, i32, i32, i32], i32);
    linker_stub!(m, "path_remove_directory", [i32, i32, i32], i32);
    linker_stub!(m, "path_rename", [i32, i32, i32, i32, i32, i32], i32);
    linker_stub!(m, "path_unlink_file", [i32, i32, i32], i32);
    linker_stub!(m, "poll_oneoff", [i32, i32, i32, i32], i32);
    linker_stub!(m, "sched_yield", [], i32);
    linker_stub!(m, "environ_get", [i32, i32], i32);
    linker_stub!(m, "fd_close", [i32], i32);
    linker_stub!(m, "fd_filestat_get", [i32, i32], i32);
    linker_stub!(m, "fd_prestat_dir_name", [i32, i32, i32], i32);
    linker_stub!(m, "fd_sync", [i32], i32);
    linker_stub!(m, "path_filestat_set_times", [i32, i32, i32, i32, i64, i64, i32], i32);
    linker_stub!(m, "fd_fdstat_set_flags", [i32, i32], i32);

    linker_stub!(m, "args_get", [i32, i32], i32, Errno::SUCCESS as i32);
    linker_stub!(m, "proc_exit", [i32], (), ());
    linker_stub!(m, "fd_fdstat_get", [i32, i32], i32, Errno::EBADFS as i32);
    linker_stub!(m, "fd_seek", [i32, i64, i32, i32], i32, Errno::EBADFS as i32);
    linker_stub!(m, "fd_prestat_get", [i32, i32], i32, Errno::EBADFS as i32);

    linker_impl!(m, "random_get", |mut caller: Caller<StoreData>, buf: i32, buf_len: i32| -> i32 { 

        log::debug!("Function random_get() called (dest buffer {:#x})", buf);

        let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
        let mem_data = mem.data_mut(&mut caller);

        let buf = buf as usize;
        let buf_len = buf_len as usize;

        mem_data[buf..buf+buf_len].fill(0xFF);

        0
    });


    linker_impl!(m, "environ_sizes_get", |mut caller: Caller<StoreData>, environ_count: i32, environ_buf_size: i32| -> i32 {

        log::debug!("Function environ_sizes_get() called (dest buffers {:#x} {:#x})", environ_count, environ_buf_size);

        let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
        let mem_data = mem.data_mut(&mut caller);

        let environ_count = environ_count as usize;
        let environ_buf_size = environ_buf_size as usize;

        mem_data[environ_count..environ_count+4].fill(0x00);
        mem_data[environ_buf_size..environ_buf_size+4].fill(0x00);

        0
    });

    linker_impl!(m, "args_sizes_get", |mut caller: Caller<StoreData>, argc: i32, argv_buf_size: i32| -> i32 {

        log::debug!("Function environ_sizes_get() called (dest buffers {:#x} {:#x})", argc, argv_buf_size);

        let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
        let mem_data = mem.data_mut(&mut caller);

        let argc = argc as usize;
        let argv_buf_size = argv_buf_size as usize;

        mem_data[argc..argc+4].fill(0x00);
        mem_data[argv_buf_size..argv_buf_size+4].fill(0x00);

        0
    });

    linker_impl!(m, "fd_write", |mut caller: Caller<StoreData>, _fd: i32, iovs: i32, _iovs_len: i32, nwritten: i32| -> i32 {
 
        //log::debug!("Function fd_write() called (fd {} iovs_len {})", fd, iovs_len);

        let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
        let mem_data = mem.data_mut(&mut caller);

        let iovs = iovs as usize;
        let nwritten = nwritten as usize;

        let buf_ptr =  u32::from_le_bytes(mem_data[iovs..iovs+4].try_into().unwrap()) as usize;
        let buf_len =  u32::from_le_bytes(mem_data[iovs+4..iovs+8].try_into().unwrap()) as usize;

        let s = core::str::from_utf8(&mem_data[buf_ptr..buf_ptr+buf_len]).unwrap();

        log::debug!("{}", s);

        mem_data[nwritten..nwritten+4].copy_from_slice((buf_len as u32).to_le_bytes().as_slice());

        0
    });
}

#[repr(i32)]
#[derive(Debug)]
enum Errno {
    SUCCESS = 0,
    EBADFS = 8,
}
