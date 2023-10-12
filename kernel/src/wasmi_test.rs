use alloc::vec;
use crate::serial_println;
use anyhow::Result;
use wasmi::{Engine, Store, Func, Caller, Module, Linker};

const WASM_CODE: &'static [u8] = include_bytes!("../../embedded_data/wasm_test.wasm");

#[derive(Debug)]
#[repr(C)]
struct AppData {
    n: u32
}

type HostState = ();

pub fn wasmi_test() -> Result<()> {

    serial_println!("Starting WASM test");

    let engine = Engine::default();
    let module = Module::new(&engine, WASM_CODE).unwrap();
    let mut store: Store<HostState> = Store::new(&engine, ());
    
    let host_get_appdata = Func::wrap(&mut store, |caller: Caller<HostState>, addr: i32| {
        let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
        let data = AppData { n: 1337 };
        unsafe {
            let ptr = &data as *const AppData as *const u8;
            let buffer = core::slice::from_raw_parts(ptr, core::mem::size_of::<AppData>());
            mem.write(caller, addr as usize, buffer).unwrap();
        };
    });

    let host_print_console = Func::wrap(&mut store, |caller: Caller<HostState>, addr: i32, len: i32| {
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

    let mut linker = <Linker<HostState>>::new(&engine);

    linker.define("env", "host_get_appdata", host_get_appdata).unwrap();
    linker.define("env", "host_print_console", host_print_console).unwrap();
    let instance = linker
        .instantiate(&mut store, &module).unwrap()
        .start(&mut store).unwrap();
    let hello = instance.get_typed_func::<(), i32>(&store, "hello").unwrap();

    serial_println!("Calling WASM");
    let res = hello.call(&mut store, ()).unwrap();

    serial_println!("Result: {}", res);

    Ok(())
}