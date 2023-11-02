use alloc::vec;
use core::mem::size_of;
use crate::serial_println;
use wasmi::{Engine, Store, Func, Caller, Module, Linker, Config, TypedFunc, AsContextMut, Instance};

use applib::SystemState;

#[derive(Debug)]
#[repr(C)]
struct AppHandle {
    n: u32
}

pub struct WasmEngine {
    engine: Engine
}

impl WasmEngine {

    pub fn new() -> Self {
        let engine = Engine::new(&Config::default().consume_fuel(false));
        WasmEngine { engine }
    }

    pub fn instantiate_app(&self, wasm_code: &[u8]) -> WasmApp {

        let module = Module::new(&self.engine, wasm_code).unwrap();
        let store_data = None;
        let mut store: Store<StoreData> = Store::new(&self.engine, store_data);
    
        let host_print_console = Func::wrap(&mut store, |caller: Caller<StoreData>, addr: i32, len: i32| {
            let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
            let len = len as usize;
            let buffer = {
                let mut buffer = vec![0u8; len];
                mem.read(caller, addr as usize, buffer.as_mut()).unwrap();
                buffer
            };
            let s = core::str::from_utf8(&buffer).unwrap();
            serial_println!("Received from WASM: {}", s);
        });

        let host_get_system_state = Func::wrap(&mut store, |caller: Caller<StoreData>, addr: i32| {
            let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
            let system_state = caller.data().as_ref().expect("System state not available");
            unsafe {
                let len = size_of::<SystemState>();
                let ptr = system_state as *const SystemState as *const u8;
                let mem_slice = core::slice::from_raw_parts(ptr, len);
                mem.write(caller, addr as usize, mem_slice).unwrap();
            }
        });

        let mut linker = <Linker<StoreData>>::new(&self.engine);
        linker.define("env", "host_print_console", host_print_console).unwrap();
        linker.define("env", "host_get_system_state", host_get_system_state).unwrap();
        let instance = linker
            .instantiate(&mut store, &module).unwrap()
            .start(&mut store).unwrap();
    
        let wasm_init = instance.get_typed_func::<(), i32>(&store, "init").unwrap();
        let wasm_step = instance.get_typed_func::<(), ()>(&store, "step").unwrap();

        let handle_addr = wasm_init
            .call(&mut store, ())
            .expect("Failed to initialize WASM app");

        WasmApp { 
            store,
            instance,
            wasm_step,
            handle_addr,
        }
    }
}

type StoreData = Option<SystemState>;

pub struct WasmApp {
    store: Store<StoreData>,
    instance: Instance,
    wasm_step: TypedFunc<(), ()>,
    handle_addr: i32,
}

impl WasmApp {
    pub fn step(&mut self, system_state: &SystemState) {

        let mem = self.instance.get_memory(&mut self.store, "memory").unwrap();
        let mut ctx = self.store.as_context_mut();

        *ctx.data_mut() = Some(system_state.clone());

        let handle = AppHandle { n: 1337 };

        unsafe {
            let ptr = &handle as *const AppHandle as *const u8;
            let buffer = core::slice::from_raw_parts(ptr, core::mem::size_of::<AppHandle>());
            mem.write(ctx, self.handle_addr as usize, buffer).unwrap();
        };

        self.wasm_step
            .call(&mut self.store, ())
            .expect("Failed to step WASM app");
    }
}
