use alloc::vec;
use core::mem::size_of;
use wasmi::{Engine, Store, Func, Caller, Module, Linker, Config, TypedFunc, AsContextMut, Instance, AsContext};

use applib::{SystemState, Framebuffer};

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
        let store_data = StoreData::new();
        let mut store: Store<StoreData> = Store::new(&self.engine, store_data);

        //
        // WASM<->System API functions
    
        let host_print_console = Func::wrap(&mut store, |caller: Caller<StoreData>, addr: i32, len: i32| {
            let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
            let len = len as usize;
            let buffer = {
                let mut buffer = vec![0u8; len];
                mem.read(caller, addr as usize, buffer.as_mut()).unwrap();
                buffer
            };
            let s = core::str::from_utf8(&buffer).unwrap();
            log::debug!("Received from WASM: {}", s);
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

        let host_set_framebuffer = Func::wrap(&mut store, |mut caller: Caller<StoreData>, addr: i32, w: i32, h: i32| {
            caller.data_mut().framebuffer = Some(WasmFramebufferDef { 
                addr: addr as usize,
                w: w as usize,
                h: h as usize,
            });
        });


        //
        // Instantiating app

        let mut linker = <Linker<StoreData>>::new(&self.engine);
        linker.define("env", "host_print_console", host_print_console).unwrap();
        linker.define("env", "host_get_system_state", host_get_system_state).unwrap();
        linker.define("env", "host_set_framebuffer", host_set_framebuffer).unwrap();
        let instance = linker
            .instantiate(&mut store, &module).unwrap()
            .start(&mut store).unwrap();
    
        let wasm_init = instance.get_typed_func::<(), ()>(&store, "init").unwrap();
        let wasm_step = instance.get_typed_func::<(), ()>(&store, "step").unwrap();


        //
        // App init

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
    h: usize,
    w: usize,
}

struct StoreData {
    system_state: Option<SystemState>,
    framebuffer: Option<WasmFramebufferDef>
}

impl StoreData {
    fn new() -> Self {
        StoreData { system_state: None, framebuffer: None }
    }
}

pub struct WasmApp {
    store: Store<StoreData>,
    instance: Instance,
    wasm_step: TypedFunc<(), ()>,
}

impl WasmApp {
    pub fn step(&mut self, system_state: &SystemState, system_fb: &mut Framebuffer) {

        let mut ctx = self.store.as_context_mut();

        ctx.data_mut().system_state = Some(system_state.clone());

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
                let fb_data = &mut mem_data[addr..addr + w*h*4];
                Framebuffer::new(fb_data, w, h)
            };

            system_fb.blend(&wasm_fb);
        }
    }
}
