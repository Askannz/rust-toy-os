use alloc::collections::BTreeMap;
use alloc::string::ToString;
use alloc::vec;
use alloc::{borrow::ToOwned, string::String};
use applib::geometry::Point2D;
use applib::{BorrowedPixels, Color};
use core::mem::size_of;
use smoltcp::iface::SocketHandle;

use rand::RngCore;
use smoltcp::wire::Ipv4Address;
use wasmi::{
    AsContext, AsContextMut, Caller, Config, Engine, Func, Instance, Linker, Memory, Module, Store,
    TypedFunc,
};

use applib::{input::InputState, FbViewMut, Framebuffer, Rect};

use crate::system::System;

pub struct WasmEngine {
    engine: Engine,
}

const STEP_FUEL: u64 = u64::MAX;

impl WasmEngine {
    pub fn new() -> Self {
        let engine = Engine::new(&Config::default().consume_fuel(true));
        WasmEngine { engine }
    }

    pub fn instantiate_app(
        &self,
        system: &mut System,
        input_state: &InputState,
        wasm_code: &[u8],
        app_name: &str,
        init_rect: &Rect,
    ) -> WasmApp {
        let module = Module::new(&self.engine, wasm_code).unwrap();
        let store_data = StoreData::new(app_name);
        let mut store: Store<StoreData> = Store::new(&self.engine, store_data);
        let mut linker = <Linker<StoreData>>::new(&self.engine);

        add_host_apis(&mut store, &mut linker);

        let instance = linker
            .instantiate(&mut store, &module)
            .unwrap()
            .start(&mut store)
            .unwrap();

        let wasm_init = instance.get_typed_func::<(), ()>(&store, "init").unwrap();
        let wasm_step = instance.get_typed_func::<(), ()>(&store, "step").unwrap();

        let mut store_wrapper = StoreWrapper { store };

        store_wrapper.with_context(system, input_state, init_rect, |store| {
            log::info!("Initializing {}", app_name);
            wasm_init
                .call(store, ())
                .expect("Failed to initialize WASM app");
        });

        WasmApp {
            store_wrapper,
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

    &mem_data[addr..addr + len]
}

fn get_wasm_mem_slice_mut<'a>(
    caller: &'a mut Caller<StoreData>,
    addr: i32,
    len: i32,
) -> &'a mut [u8] {
    let mem = get_linear_memory(caller);

    let mem_data = mem.data_mut(caller);
    let len = len as usize;
    let addr = addr as usize;

    &mut mem_data[addr..addr + len]
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
    caller
        .get_export("memory")
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

struct SocketsStore {
    sockets: BTreeMap<i32, SocketHandle>,
    next_id: i32,
}

impl SocketsStore {
    fn new() -> Self {
        Self {
            sockets: BTreeMap::new(),
            next_id: 0,
        }
    }

    fn add_handle(&mut self, handle: SocketHandle) -> i32 {
        let new_id = self.next_id;
        self.next_id += 1;
        self.sockets.insert(new_id, handle);
        new_id
    }

    fn get_handle(&self, handle_id: i32) -> Option<SocketHandle> {
        self.sockets.get(&handle_id).cloned()
    }
}

struct StoreWrapper {
    store: Store<StoreData>,
}

impl StoreWrapper {
    fn with_context<F, T>(
        &mut self,
        system: &mut System,
        input_state: &InputState,
        win_rect: &Rect,
        mut func: F,
    ) -> T where
        F: FnMut(&mut Store<StoreData>) -> T,
    {
        self.store.set_fuel(STEP_FUEL).unwrap();

        self.store.as_context_mut().data_mut().step_context = Some(StepContext {
            // reference -> raw pointer conversions here
            system,
            input_state,

            win_rect: win_rect.clone(),
            timings: BTreeMap::new(),
        });

        let res = func(&mut self.store);

        self.store.as_context_mut().data_mut().step_context = None;

        res
    }

    fn get_framebuffer(&self, instance: &Instance) -> Option<Framebuffer<BorrowedPixels>> {
        let wasm_fb_def = self
            .store
            .as_context()
            .data()
            .framebuffer
            .clone()?;

        let mem = instance.get_memory(&self.store, "memory").unwrap();
        let ctx = self.store.as_context();
        let mem_data = mem.data(ctx);

        let wasm_fb = {
            let WasmFramebufferDef { addr, w, h } = wasm_fb_def;
            let fb_data = &mem_data[addr..addr + (w * h * 4) as usize];
            let fb_data = unsafe { 
                let (head, body, tail) = fb_data.align_to::<Color>();
                assert_eq!(head.len(), 0);
                assert_eq!(tail.len(), 0);
                body
            };
            Framebuffer::<BorrowedPixels>::new(fb_data, w, h)
        };

        Some(wasm_fb)
    }
}

struct StoreData {
    app_name: String,
    framebuffer: Option<WasmFramebufferDef>,
    sockets_store: SocketsStore,
    step_context: Option<StepContext>,
}

struct StepContext {
    system: *mut System,
    input_state: *const InputState,
    win_rect: Rect,
    timings: BTreeMap<String, u64>,
}

struct StepContextView<'a> {
    system: &'a mut System,
    input_state: &'a InputState,
    win_rect: &'a Rect,
    timings: &'a mut BTreeMap<String, u64>,
}

impl StoreData {
    fn new(app_name: &str) -> Self {
        StoreData {
            app_name: app_name.to_owned(),
            framebuffer: None,
            sockets_store: SocketsStore::new(),
            step_context: None,
        }
    }

    fn with_step_context<F, T>(&mut self, mut func: F) -> T
    where
        F: FnMut(StepContextView) -> T,
    {
        let step_context = self.step_context.as_mut().expect("No StepContext set");

        let step_context_view = StepContextView {
            // Safety: thanks to the StoreDataWrapper scope, those pointers should always be valid
            system: unsafe { step_context.system.as_mut().unwrap() },
            input_state: unsafe { step_context.input_state.as_ref().unwrap() },

            win_rect: &step_context.win_rect,
            timings: &mut step_context.timings,
        };

        func(step_context_view)
    }
}

pub struct WasmApp {
    store_wrapper: StoreWrapper,
    instance: Instance,
    wasm_step: TypedFunc<(), ()>,
}

impl WasmApp {

    pub fn step(
        &mut self,
        system: &mut System,
        input_state: &InputState,
        win_rect: &Rect,
        is_foreground: bool,
    ) -> Result<(), anyhow::Error> {

        let relative_input_state = {
            let mut input_state = input_state.clone();
            if !is_foreground {
                input_state.clear_events();
            }
            let (ox, oy) = win_rect.origin();
            input_state.change_origin(Point2D { x: ox, y: oy });
            input_state
        };

        self.store_wrapper
            .with_context(system, &relative_input_state, win_rect, |mut store| {
                self.wasm_step.call(&mut store, ())
            })
            .map_err(|wasm_err| anyhow::format_err!(wasm_err))?;


        // let mem = self.instance.get_memory(&store, "memory").unwrap();
        // let mem_size = mem.size(store.as_context()) * 65_536;
        // log::debug!("{}: {}MB", store.data().app_name, mem_size / 1_000_000);

        Ok(())
    }

    pub fn get_framebuffer(&self) -> Option<Framebuffer<BorrowedPixels>> {
        self.store_wrapper.get_framebuffer(&self.instance)
    }
}

// fn debug_stall(t0: f64, t1: f64, fu0: u64, fu1: u64, store_data: &StoreData) {
//     const STALL_THRESHOLD: f64 = 1000.0 / 60.0;

//     if t1 - t0 > STALL_THRESHOLD {
//         let total_consumed = fu0 - fu1;
//         let total_consumed_f = total_consumed as f64;

//         let lines: Vec<String> = store_data
//             .timings
//             .iter()
//             .map(|(k, v)| {
//                 format!(
//                     "  {}: {}u ({:.1}%)",
//                     k,
//                     v,
//                     100f64 * (*v as f64) / total_consumed_f
//                 )
//             })
//             .collect();

//         log::warn!(
//             "STALL ({:.0}ms > {:.0}ms)\n\
//             Total fuel consumed: {}u\n\
//             {}",
//             t1 - t0,
//             STALL_THRESHOLD,
//             total_consumed,
//             lines.join("\n")
//         );
//     }
// }

fn add_host_apis(mut store: &mut Store<StoreData>, linker: &mut Linker<StoreData>) {

    // This works but is sadly not enough to display a backtrace, not sure why
    const ENV_VARS: [&str; 1] = ["RUST_BACKTRACE=full"];

    macro_rules! linker_impl {
        ($module:expr, $name:expr, $func:expr) => {
            linker
                .define($module, $name, Func::wrap(&mut store, $func))
                .unwrap();
        };
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
    linker_stub!(
        m,
        "path_open",
        [i32, i32, i32, i32, i32, i64, i64, i32, i32],
        i32
    );
    linker_stub!(m, "path_readlink", [i32, i32, i32, i32, i32, i32], i32);
    linker_stub!(m, "path_remove_directory", [i32, i32, i32], i32);
    linker_stub!(m, "path_rename", [i32, i32, i32, i32, i32, i32], i32);
    linker_stub!(m, "path_unlink_file", [i32, i32, i32], i32);
    linker_stub!(m, "poll_oneoff", [i32, i32, i32, i32], i32);
    linker_stub!(m, "sched_yield", [], i32);
    linker_stub!(m, "fd_close", [i32], i32);
    linker_stub!(m, "fd_filestat_get", [i32, i32], i32);
    linker_stub!(m, "fd_prestat_dir_name", [i32, i32, i32], i32);
    linker_stub!(m, "fd_sync", [i32], i32);
    linker_stub!(
        m,
        "path_filestat_set_times",
        [i32, i32, i32, i32, i64, i64, i32],
        i32
    );
    linker_stub!(m, "fd_fdstat_set_flags", [i32, i32], i32);

    //
    // WASMI stubs (with return value)

    linker_stub!(m, "args_get", [i32, i32], i32, Errno::SUCCESS as i32);
    linker_stub!(m, "proc_exit", [i32], (), ());
    linker_stub!(m, "fd_fdstat_get", [i32, i32], i32, Errno::EBADFS as i32);
    linker_stub!(
        m,
        "fd_seek",
        [i32, i64, i32, i32],
        i32,
        Errno::EBADFS as i32
    );
    linker_stub!(m, "fd_prestat_get", [i32, i32], i32, Errno::EBADFS as i32);

    //
    // WASMI implementations

    linker_impl!(m, "clock_time_get", |mut caller: Caller<StoreData>,
                                       clock_id: i32,
                                       precision: i64,
                                       time: i32|
     -> i32 {
        let buf = time as usize;

        log::debug!(
            "Function clock_time_get() called (dest buffer {:#x} clock_id {:#x} precision {})",
            buf,
            clock_id,
            precision
        );

        let t = caller.data_mut().with_step_context(|step_context| {
            (step_context.system.clock.time() * 1e9) as u64 // Not sure about the 1e9
        });

        let mem = get_linear_memory(&caller);
        let mem_data = mem.data_mut(&mut caller);

        let data = t.to_le_bytes();
        mem_data[buf..buf + 8].copy_from_slice(&data);

        0
    });

    linker_impl!(m, "random_get", |mut caller: Caller<StoreData>,
                                   buf: i32,
                                   buf_len: i32|
     -> i32 {
        log::debug!("Function random_get() called (dest buffer {:#x})", buf);

        let buf = buf as usize;
        let buf_len = buf_len as usize;

        let mut rand_bytes = vec![0u8; buf_len];

        caller.data_mut().with_step_context(|step_context| {
            step_context.system.rng.fill_bytes(&mut rand_bytes);
        });

        let mem = get_linear_memory(&caller);
        let mem_data = mem.data_mut(&mut caller);

        mem_data[buf..buf + buf_len].copy_from_slice(&rand_bytes);

        0
    });

    linker_impl!(m, "environ_sizes_get", |mut caller: Caller<StoreData>,
                                          environ_count: i32,
                                          environ_buf_size: i32|
     -> i32 {
        log::debug!(
            "Function environ_sizes_get() called (dest buffers {:#x} {:#x})",
            environ_count,
            environ_buf_size
        );

        let mem = get_linear_memory(&caller);
        let mem_data = mem.data_mut(&mut caller);

        let environ_count = environ_count as usize;
        let environ_buf_size = environ_buf_size as usize;

        let n_env_vars = ENV_VARS.len() as u32;
        let string_data_size: u32 = ENV_VARS.iter().map(|s| s.len() as u32 + 1).sum();

        mem_data[environ_count..environ_count + 4].copy_from_slice(&u32::to_le_bytes(n_env_vars));
        mem_data[environ_buf_size..environ_buf_size + 4].copy_from_slice(&u32::to_le_bytes(string_data_size));

        0
    });

    linker_impl!(m, "environ_get", |mut caller: Caller<StoreData>,
                                          environ: i32,
                                          environ_buf: i32|
     -> i32 {
        log::debug!(
            "Function environ_get() called (dest buffers {:#x} {:#x})",
            environ,
            environ_buf
        );

        let mem = get_linear_memory(&caller);
        let mem_data = mem.data_mut(&mut caller);

        let mut p_addr = environ as usize;
        let mut str_addr = environ_buf as usize;

        for env_str in ENV_VARS.iter() {

            let p_bytes = &u32::to_le_bytes(str_addr as u32);
            mem_data[p_addr..p_addr+p_bytes.len()].copy_from_slice(p_bytes);
            p_addr += p_bytes.len();

            let s_bytes = env_str.as_bytes();
            mem_data[str_addr..str_addr+s_bytes.len()].copy_from_slice(s_bytes);
            str_addr += s_bytes.len();
            mem_data[str_addr] = 0;
            str_addr += 1;
        }

        0
    });

    linker_impl!(m, "args_sizes_get", |mut caller: Caller<StoreData>,
                                       argc: i32,
                                       argv_buf_size: i32|
     -> i32 {
        log::debug!(
            "Function environ_sizes_get() called (dest buffers {:#x} {:#x})",
            argc,
            argv_buf_size
        );

        let mem = get_linear_memory(&caller);
        let mem_data = mem.data_mut(&mut caller);

        let argc = argc as usize;
        let argv_buf_size = argv_buf_size as usize;

        mem_data[argc..argc + 4].fill(0x00);
        mem_data[argv_buf_size..argv_buf_size + 4].fill(0x00);

        0
    });

    linker_impl!(m, "fd_write", |mut caller: Caller<StoreData>,
                                 _fd: i32,
                                 iovs: i32,
                                 _iovs_len: i32,
                                 nwritten: i32|
     -> i32 {
        //log::debug!("Function fd_write() called (fd {} iovs_len {})", fd, iovs_len);

        let mem = get_linear_memory(&caller);
        let mem_data = mem.data_mut(&mut caller);

        let iovs = iovs as usize;
        let nwritten = nwritten as usize;

        let buf_ptr = u32::from_le_bytes(mem_data[iovs..iovs + 4].try_into().unwrap()) as usize;
        let buf_len = u32::from_le_bytes(mem_data[iovs + 4..iovs + 8].try_into().unwrap()) as usize;

        let s = core::str::from_utf8(&mem_data[buf_ptr..buf_ptr + buf_len]).unwrap();

        log::debug!("{}", s);

        mem_data[nwritten..nwritten + 4].copy_from_slice((buf_len as u32).to_le_bytes().as_slice());

        0
    });

    //
    // APIs specific to this particular WASM environment

    let m = "env";

    linker_impl!(
        m,
        "host_print_console",
        |caller: Caller<StoreData>, addr: i32, len: i32| {
            let ctx = caller.as_context();
            let app_name = &ctx.data().app_name;
            let mem_slice = get_wasm_mem_slice(&caller, addr, len);

            let s = core::str::from_utf8(mem_slice)
                .expect("Not UTF-8")
                .trim_end();

            log::debug!("{}: {}", app_name, s);
        }
    );

    linker_impl!(m, "host_log", |caller: Caller<StoreData>,
                                 addr: i32,
                                 len: i32,
                                 level| {
        let mem_slice = get_wasm_mem_slice(&caller, addr, len);

        let s = core::str::from_utf8(mem_slice)
            .expect("Not UTF-8")
            .trim_end();

        match level {
            1 => log::error!("{}", s),
            2 => log::warn!("{}", s),
            3 => log::info!("{}", s),
            4 => log::debug!("{}", s),
            _ => log::trace!("{}", s),
        };
    });

    linker_impl!(
        m,
        "host_get_input_state",
        |mut caller: Caller<StoreData>, addr: i32| {
            let system_state = caller
                .data_mut()
                .with_step_context(|step_context| step_context.input_state.clone());

            write_to_wasm_mem(&mut caller, addr, &system_state);
        }
    );

    linker_impl!(
        m,
        "host_get_win_rect",
        |mut caller: Caller<StoreData>, addr: i32| {
            let win_rect = caller
                .data_mut()
                .with_step_context(|step_context| step_context.win_rect.clone());
            write_to_wasm_mem(&mut caller, addr, &win_rect);
        }
    );

    linker_impl!(
        m,
        "host_set_framebuffer",
        |mut caller: Caller<StoreData>, addr: i32, w: i32, h: i32| {
            caller.data_mut().framebuffer = Some(WasmFramebufferDef {
                addr: addr as usize,
                w: w as u32,
                h: h as u32,
            });
        }
    );

    linker_impl!(m, "host_tcp_connect", |mut caller: Caller<StoreData>,
                                         ip_addr: i32,
                                         port: i32|
     -> i32 {
        let mut try_connect = || -> anyhow::Result<i32> {
            let ip_bytes = ip_addr.to_le_bytes();
            let port: u16 = port.try_into().expect("Invalid port value");

            let socket_handle = caller.data_mut().with_step_context(|step_context| {
                step_context
                    .system
                    .tcp_stack
                    .connect(Ipv4Address(ip_bytes), port)
            })?;

            let handle_id = caller.data_mut().sockets_store.add_handle(socket_handle);
            Ok(handle_id)
        };

        match try_connect() {
            Ok(handle_id) => handle_id,
            Err(err) => {
                log::error!("{}", err);
                -1
            }
        }
    });

    linker_impl!(m, "host_tcp_may_send", |mut caller: Caller<StoreData>,
                                          handle_id: i32|
     -> i32 {
        let socket_handle = caller
            .data_mut()
            .sockets_store
            .get_handle(handle_id)
            .expect("No TCP connection");

        let ret: bool = caller.data_mut().with_step_context(|step_context| {
            step_context.system.tcp_stack.may_send(socket_handle).into()
        });

        ret.into()
    });

    linker_impl!(m, "host_tcp_may_recv", |mut caller: Caller<StoreData>,
                                          handle_id: i32|
     -> i32 {
        let socket_handle = caller
            .data_mut()
            .sockets_store
            .get_handle(handle_id)
            .expect("No TCP connection");

        let ret: bool = caller.data_mut().with_step_context(|step_context| {
            step_context.system.tcp_stack.may_recv(socket_handle).into()
        });

        ret.into()
    });

    linker_impl!(m, "host_tcp_write", |mut caller: Caller<StoreData>,
                                       addr: i32,
                                       len: i32,
                                       handle_id: i32|
     -> i32 {
        let mut try_write = || -> anyhow::Result<i32> {
            let buf = get_wasm_mem_slice(&mut caller, addr, len).to_vec();

            let socket_handle = caller
                .data_mut()
                .sockets_store
                .get_handle(handle_id)
                .expect("No TCP connection");

            let written_len = caller.data_mut().with_step_context(|step_context| {
                step_context.system.tcp_stack.write(socket_handle, &buf)
            })?;

            let written_len: i32 = written_len.try_into().map_err(anyhow::Error::msg)?;

            Ok(written_len)
        };

        match try_write() {
            Ok(written_len) => written_len,
            Err(err) => {
                log::error!("{}", err);
                -1
            }
        }
    });

    linker_impl!(m, "host_tcp_read", |mut caller: Caller<StoreData>,
                                      addr: i32,
                                      len: i32,
                                      handle_id: i32|
     -> i32 {
        let mut try_read = || -> anyhow::Result<i32> {
            let len = len as usize;
            let addr = addr as usize;

            let mut buf = vec![0u8; len];

            let read_len: usize = {
                let socket_handle = caller
                    .data_mut()
                    .sockets_store
                    .get_handle(handle_id)
                    .expect("No TCP connection");
                caller.data_mut().with_step_context(|step_context| {
                    step_context.system.tcp_stack.read(socket_handle, &mut buf)
                })?
            };

            let mem = get_linear_memory(&caller);
            let mem_data = mem.data_mut(&mut caller);

            mem_data[addr..addr + read_len].copy_from_slice(&buf[..read_len]);

            let read_len: i32 = read_len.try_into().unwrap();

            Ok(read_len)
        };

        match try_read() {
            Ok(read_len) => read_len,
            Err(err) => {
                log::error!("{}", err);
                -1
            }
        }
    });

    linker_impl!(
        m,
        "host_tcp_close",
        |mut caller: Caller<StoreData>, handle_id: i32| {
            let socket_handle = caller
                .data_mut()
                .sockets_store
                .get_handle(handle_id)
                .expect("No TCP connection");

            caller.data_mut().with_step_context(|step_context| {
                step_context.system.tcp_stack.close(socket_handle)
            })
        }
    );

    linker_impl!(
        m,
        "host_get_time",
        |mut caller: Caller<StoreData>, buf: i32| {
            let buf = buf as usize;

            let t = caller
                .data_mut()
                .with_step_context(|step_context| step_context.system.clock.time());

            let mem = get_linear_memory(&caller);
            let mem_data = mem.data_mut(&mut caller);

            let data = t.to_le_bytes();
            mem_data[buf..buf + 8].copy_from_slice(&data);
        }
    );

    linker_impl!(
        m,
        "host_get_stylesheet",
        |mut caller: Caller<StoreData>, addr: i32| {
            let stylesheet = caller
                .data_mut()
                .with_step_context(|step_context| step_context.system.stylesheet.clone());

            write_to_wasm_mem(&mut caller, addr, &stylesheet);
        }
    );

    linker_impl!(
        m,
        "host_get_consumed_fuel",
        |mut caller: Caller<StoreData>, consumed_addr: i32| {
            let remaining = caller.get_fuel().expect("Fuel metering disabled");
            let consumed = STEP_FUEL - remaining;
            write_to_wasm_mem(&mut caller, consumed_addr, &consumed.to_le_bytes());
        }
    );

    linker_impl!(
        m,
        "host_save_timing",
        |mut caller: Caller<StoreData>, key_addr: i32, key_len: i32, consumed_addr: i32| {
            let key_buf = get_wasm_mem_slice(&mut caller, key_addr, key_len);
            let key = core::str::from_utf8(key_buf)
                .expect("Invalid key")
                .to_string();

            let consumed_buf: [u8; 8] = get_wasm_mem_slice(&mut caller, consumed_addr, 8)
                .try_into()
                .unwrap();
            let consumed: u64 = u64::from_le_bytes(consumed_buf);

            caller.data_mut().with_step_context(|step_context| {
                step_context.timings.insert(key.clone(), consumed)
            });
        }
    );

    linker_impl!(
        m,
        "host_qemu_dump",
        |caller: Caller<StoreData>, addr: i32, len: i32| {
            let mem_slice = get_wasm_mem_slice(&caller, addr, len);
            let buf = mem_slice.to_vec();

            let phys_addr = buf.leak().as_mut_ptr() as u64;

            log::debug!(
                "QEMU DUMP: pmemsave 0x{:x} {} pmem_dump.bin",
                phys_addr,
                len
            );
        }
    );
}

#[repr(i32)]
#[derive(Debug)]
enum Errno {
    SUCCESS = 0,
    EBADFS = 8,
}
