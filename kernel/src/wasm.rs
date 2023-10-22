use alloc::vec;
use crate::serial_println;
use wasmi::{Engine, Store, Func, Caller, Module, Linker, Config, TypedFunc};

#[derive(Debug)]
#[repr(C)]
struct AppData {
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
        let mut store: Store<()> = Store::new(&self.engine, ());

        // TEST
        let host_get_appdata = Func::wrap(&mut store, |caller: Caller<()>, addr: i32| {
            let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
            let data = AppData { n: 1337 };
            unsafe {
                let ptr = &data as *const AppData as *const u8;
                let buffer = core::slice::from_raw_parts(ptr, core::mem::size_of::<AppData>());
                mem.write(caller, addr as usize, buffer).unwrap();
            };
        });
    
        let host_print_console = Func::wrap(&mut store, |caller: Caller<()>, addr: i32, len: i32| {
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

        let mut linker = <Linker<()>>::new(&self.engine);
        linker.define("env", "host_get_appdata", host_get_appdata).unwrap();
        linker.define("env", "host_print_console", host_print_console).unwrap();
        let instance = linker
            .instantiate(&mut store, &module).unwrap()
            .start(&mut store).unwrap();
    
        let wasm_init = instance.get_typed_func::<(), ()>(&store, "init").unwrap();
        let wasm_step = instance.get_typed_func::<(), ()>(&store, "step").unwrap();

        wasm_init
            .call(&mut store, ())
            .expect("Failed to initialize WASM app");

        WasmApp { 
            store,
            wasm_step,
        }
    }
}

pub struct WasmApp {
    store: Store<()>,
    wasm_step: TypedFunc<(), ()>,
}

impl WasmApp {
    pub fn step(&mut self) {
        self.wasm_step
            .call(&mut self.store, ())
            .expect("Failed to step WASM app");
    }
}
