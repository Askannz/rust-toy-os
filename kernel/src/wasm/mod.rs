use core::mem::size_of;
use core::cell::RefCell;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use alloc::rc::Rc;
use alloc::format;
use alloc::{string::String, borrow::ToOwned};
use alloc::collections::BTreeMap;
use smoltcp::iface::SocketHandle;

use smoltcp::wire::{Ipv4Address};
use wasmi::{Engine, Store, Func, Caller, Module, Linker, Config, TypedFunc, AsContextMut, Instance, AsContext, Memory};


use applib::{SystemState, Framebuffer, Rect};

use crate::network::TcpStack;
use crate::time::SystemClock;

pub struct WasmEngine {
    engine: Engine,
}

impl WasmEngine {

    pub fn new() -> Self {
        let engine = Engine::new(&Config::default().consume_fuel(true));
        WasmEngine { engine }
    }

    pub fn instantiate_app(&self, tcp_stack: Rc<RefCell<TcpStack>>, wasm_code: &[u8], app_name: &str, init_rect: &Rect) -> WasmApp {

        let module = Module::new(&self.engine, wasm_code).unwrap();
        let store_data = StoreData::new(tcp_stack, app_name, init_rect);
        let mut store: Store<StoreData> = Store::new(&self.engine, store_data);
        let mut linker = <Linker<StoreData>>::new(&self.engine);

        add_host_apis(&mut store, &mut linker);

        let instance = linker
            .instantiate(&mut store, &module).unwrap()
            .start(&mut store).unwrap();
    
        let wasm_init = instance.get_typed_func::<(), ()>(&store, "init").unwrap();
        let wasm_step = instance.get_typed_func::<(), ()>(&store, "step").unwrap();

        store.add_fuel(u64::MAX).unwrap();


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

fn get_wasm_mem_slice<'a>(caller: &'a Caller<StoreData>, addr: i32, len: i32) -> &'a [u8] {

    let mem = get_linear_memory(caller);

    let mem_data = mem.data(caller);
    let len = len as usize;
    let addr = addr as usize;

    &mem_data[addr..addr+len]
}

fn get_wasm_mem_slice_mut<'a>(caller: &'a mut Caller<StoreData>, addr: i32, len: i32) -> &'a mut [u8] {

    let mem = get_linear_memory(caller);

    let mem_data = mem.data_mut(caller);
    let len = len as usize;
    let addr = addr as usize;

    &mut mem_data[addr..addr+len]
}

fn write_to_wasm_mem<'a, T: Sized>(caller: &'a mut Caller<StoreData>, addr: i32, data: &T) {

    let mem = get_linear_memory(caller);

    unsafe {
        let len = size_of::<T>();
        let ptr = data as *const T as *const u8;
        let mem_slice = core::slice::from_raw_parts(ptr, len);
        mem.write(caller, addr as usize, mem_slice)
            .expect("Failed to write to WASM memory");
    }
}


fn get_linear_memory(caller: &Caller<StoreData>) -> Memory {
    caller.get_export("memory")
        .expect("No WASM memory export")
        .into_memory()
        .expect("Not a linear memory")
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
    socket_handle: Option<SocketHandle>,

    timings: BTreeMap<String, u64>,
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
            timings: BTreeMap::new(),
        }
    }
}

pub struct WasmApp {
    store: Store<StoreData>,
    instance: Instance,
    wasm_step: TypedFunc<(), ()>,
}

impl WasmApp {
    pub fn step(&mut self, system_state: &SystemState, clock: &SystemClock, system_fb: &mut Framebuffer, win_rect: &Rect) {

        let mut ctx = self.store.as_context_mut();
        let data_mut = ctx.data_mut();
        
        data_mut.system_state = Some(system_state.clone());
        data_mut.win_rect = win_rect.clone();
        data_mut.timings.clear();

        let t0 = clock.time();
        let fu0 = self.store.fuel_consumed().unwrap();
        self.wasm_step
            .call(&mut self.store, ())
            .expect("Failed to step WASM app");
        let t1 = clock.time();
        let fu1 = self.store.fuel_consumed().unwrap();

        debug_stall(t0, t1, fu0, fu1, self.store.data());


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

            system_fb.copy_from_fb(&wasm_fb, &wasm_fb.shape_as_rect(), &win_rect, false);
        }
    }
}

fn debug_stall(t0: f64, t1: f64, fu0: u64, fu1: u64, store_data: &StoreData) {

    const STALL_THRESHOLD: f64 = 100f64;

    if t1 - t0 > STALL_THRESHOLD {

        let total_consumed = fu1 - fu0;
        let total_consumed_f = total_consumed as f64;

        let lines: Vec<String> = store_data.timings.iter().map(|(k, v)| {
            format!("  {}: {}u ({:.1}%)", k, v, 100f64 * (*v as f64) / total_consumed_f)
        })
        .collect();

        log::warn!(
            "STALL ({:.0}ms > {:.0}ms)\n\
            Total fuel consumed: {}u\n\
            {}",
            t1 - t0, STALL_THRESHOLD,
            total_consumed,
            lines.join("\n")
        );
    }
}

fn add_host_apis(mut store: &mut Store<StoreData>, linker: &mut Linker<StoreData>) {

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

    //
    // Argc/argv stub

    linker_stub!("__main_argc_argv", "env", [i32, i32], i32);


    //
    // WASMI stubs (unimplemented)

    let m = "wasi_snapshot_preview1";

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


    //
    // WASMI stubs (with return value)

    linker_stub!(m, "args_get", [i32, i32], i32, Errno::SUCCESS as i32);
    linker_stub!(m, "proc_exit", [i32], (), ());
    linker_stub!(m, "fd_fdstat_get", [i32, i32], i32, Errno::EBADFS as i32);
    linker_stub!(m, "fd_seek", [i32, i64, i32, i32], i32, Errno::EBADFS as i32);
    linker_stub!(m, "fd_prestat_get", [i32, i32], i32, Errno::EBADFS as i32);


    //
    // WASMI implementations

    linker_impl!(m, "clock_time_get", |mut caller: Caller<StoreData>, clock_id: i32, precision: i64, time: i32| -> i32 {

        let buf = time as usize;

        log::debug!("Function clock_time_get() called (dest buffer {:#x} clock_id {:#x} precision {})", buf, clock_id, precision);

        let system_state = caller.data()
            .system_state
            .as_ref().expect("System state not available");

        let t = (system_state.time * 1e9) as u64;

        let mem = get_linear_memory(&caller);
        let mem_data = mem.data_mut(&mut caller);

        let data = t.to_le_bytes();
        mem_data[buf..buf+8].copy_from_slice(&data);

        0
    });

    linker_impl!(m, "random_get", |mut caller: Caller<StoreData>, buf: i32, buf_len: i32| -> i32 { 

        log::debug!("Function random_get() called (dest buffer {:#x})", buf);

        let mem = get_linear_memory(&caller);
        let mem_data = mem.data_mut(&mut caller);

        let buf = buf as usize;
        let buf_len = buf_len as usize;

        mem_data[buf..buf+buf_len].fill(0xFF);

        0
    });


    linker_impl!(m, "environ_sizes_get", |mut caller: Caller<StoreData>, environ_count: i32, environ_buf_size: i32| -> i32 {

        log::debug!("Function environ_sizes_get() called (dest buffers {:#x} {:#x})", environ_count, environ_buf_size);

        let mem = get_linear_memory(&caller);
        let mem_data = mem.data_mut(&mut caller);

        let environ_count = environ_count as usize;
        let environ_buf_size = environ_buf_size as usize;

        mem_data[environ_count..environ_count+4].fill(0x00);
        mem_data[environ_buf_size..environ_buf_size+4].fill(0x00);

        0
    });

    linker_impl!(m, "args_sizes_get", |mut caller: Caller<StoreData>, argc: i32, argv_buf_size: i32| -> i32 {

        log::debug!("Function environ_sizes_get() called (dest buffers {:#x} {:#x})", argc, argv_buf_size);

        let mem = get_linear_memory(&caller);
        let mem_data = mem.data_mut(&mut caller);

        let argc = argc as usize;
        let argv_buf_size = argv_buf_size as usize;

        mem_data[argc..argc+4].fill(0x00);
        mem_data[argv_buf_size..argv_buf_size+4].fill(0x00);

        0
    });

    linker_impl!(m, "fd_write", |mut caller: Caller<StoreData>, _fd: i32, iovs: i32, _iovs_len: i32, nwritten: i32| -> i32 {
 
        //log::debug!("Function fd_write() called (fd {} iovs_len {})", fd, iovs_len);

        let mem = get_linear_memory(&caller);
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


    //
    // APIs specific to this particular WASM environment

    let m = "env";

    linker_impl!(m, "host_print_console", |caller: Caller<StoreData>, addr: i32, len: i32| {
        let ctx = caller.as_context();
        let app_name = &ctx.data().app_name;
        let mem_slice = get_wasm_mem_slice(&caller, addr, len);

        let s = core::str::from_utf8(mem_slice)
            .expect("Not UTF-8")
            .trim_end();

        log::debug!("{}: {}", app_name, s);
    });

    linker_impl!(m, "host_get_system_state", |mut caller: Caller<StoreData>, addr: i32| {

        let system_state = caller.data()
            .system_state
            .as_ref()
            .expect("System state not available")
            .clone();

        write_to_wasm_mem(&mut caller, addr, &system_state);
    });

    linker_impl!(m, "host_get_win_rect", |mut caller: Caller<StoreData>, addr: i32| {
        let win_rect = caller.data().win_rect.clone();
        write_to_wasm_mem(&mut caller, addr, &win_rect);
    });

    linker_impl!(m, "host_set_framebuffer", |mut caller: Caller<StoreData>, addr: i32, w: i32, h: i32| {
        caller.data_mut().framebuffer = Some(WasmFramebufferDef { 
            addr: addr as usize,
            w: w as u32,
            h: h as u32,
        });
    });

    // TODO: proper socket handles rather than a single global socket

    linker_impl!(m, "host_tcp_connect", |mut caller: Caller<StoreData>, ip_addr: i32, port: i32| {
        let data_mut = caller.data_mut();
        let mut tcp_stack = data_mut.tcp_stack.borrow_mut();

        let ip_bytes = ip_addr.to_le_bytes();
        let port: u16 = port.try_into().expect("Invalid port value");

        let socket_handle = tcp_stack.connect(Ipv4Address(ip_bytes), port);
        data_mut.socket_handle = Some(socket_handle);
    });

    linker_impl!(m, "host_tcp_may_send", |caller: Caller<StoreData>| -> i32 {
        let data = caller.data();
        let tcp_stack = data.tcp_stack.borrow_mut();
        let socket_handle = data.socket_handle.expect("No TCP connection");
        tcp_stack.may_send(socket_handle).into()
    });

    linker_impl!(m, "host_tcp_may_recv", |caller: Caller<StoreData>| -> i32 {
        let data = caller.data();
        let tcp_stack = data.tcp_stack.borrow_mut();
        let socket_handle = data.socket_handle.expect("No TCP connection");
        tcp_stack.may_recv(socket_handle).into()
    });

    linker_impl!(m, "host_tcp_write", |mut caller: Caller<StoreData>, addr: i32, len: i32| -> i32 {

        let buf = get_wasm_mem_slice(&mut caller, addr, len).to_vec();

        let data_mut = caller.data_mut();
        let mut tcp_stack = data_mut.tcp_stack.borrow_mut();

        let socket_handle = data_mut.socket_handle.expect("No TCP connection");

        let written_len = tcp_stack.write(socket_handle, &buf);

        let written_len: i32 = written_len.try_into().unwrap();

        written_len
    });

    linker_impl!(m, "host_tcp_read", |mut caller: Caller<StoreData>, addr: i32, len: i32| -> i32 {

        let len = len as usize;
        let addr = addr as usize;

        let mut buf = vec![0; len];

        let read_len: usize = {
            let data_mut = caller.data_mut();
            let mut tcp_stack = data_mut.tcp_stack.borrow_mut();
            let socket_handle = data_mut.socket_handle.expect("No TCP connection");
            tcp_stack.read(socket_handle, &mut buf)
        };

        let mem = get_linear_memory(&caller);
        let mem_data = mem.data_mut(&mut caller);

        mem_data[addr..addr+read_len].copy_from_slice(&buf[..read_len]);

        let read_len: i32 = read_len.try_into().unwrap();

        read_len
    });

    linker_impl!(m, "host_get_consumed_fuel", |mut caller: Caller<StoreData>, consumed_addr: i32| {
        let consumed = caller.fuel_consumed().expect("Fuel metering disabled");
        write_to_wasm_mem(&mut caller, consumed_addr, &consumed.to_le_bytes());
    });

    linker_impl!(m, "host_save_timing", |mut caller: Caller<StoreData>, key_addr: i32, key_len: i32, consumed_addr: i32| {

        let key_buf = get_wasm_mem_slice(&mut caller, key_addr, key_len);
        let key = core::str::from_utf8(key_buf).expect("Invalid key").to_string();

        let consumed_buf: [u8; 8] = get_wasm_mem_slice(&mut caller, consumed_addr, 8).try_into().unwrap();
        let consumed: u64 = u64::from_le_bytes(consumed_buf);

        caller.data_mut().timings.insert(key, consumed);
    });

}

#[repr(i32)]
#[derive(Debug)]
enum Errno {
    SUCCESS = 0,
    EBADFS = 8,
}
