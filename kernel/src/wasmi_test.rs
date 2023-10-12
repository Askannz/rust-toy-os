use alloc::string::String;
use core::mem;
use crate::serial_println;
use anyhow::{anyhow, Result};
//use wasmi::*;
use wasmi::{Engine, Store, Func, Caller, Module, Linker};

const WASM_CODE: &'static [u8] = include_bytes!("../../embedded_data/wasm_test.wasm");

pub fn wasmi_test() -> Result<()> {
    // First step is to create the Wasm execution engine with some config.
    // In this example we are using the default configuration.
    let engine = Engine::default();

    let module = Module::new(&engine, WASM_CODE).unwrap();

    // All Wasm objects operate within the context of a `Store`.
    // Each `Store` has a type parameter to store host-specific data,
    // which in this case we are using `42` for.
    type HostState = u32;
    let mut store = Store::new(&engine, 42);
    let host_printouille = Func::wrap(&mut store, |mut caller: Caller<HostState>| {
        serial_println!("Hello from WASM!")
    });

    // In order to create Wasm module instances and link their imports
    // and exports we require a `Linker`.
    let mut linker = <Linker<HostState>>::new(&engine);
    // Instantiation of a Wasm module requires defining its imports and then
    // afterwards we can fetch exports by name, as well as asserting the
    // type signature of the function with `get_typed_func`.
    //
    // Also before using an instance created this way we need to start it.
    linker.define("env", "printouille", host_printouille).unwrap();
    let instance = linker
        .instantiate(&mut store, &module).unwrap()
        .start(&mut store).unwrap();
    let hello = instance.get_typed_func::<(), i32>(&store, "hello").unwrap();

    let memory = instance.get_memory(&mut store, "memory").unwrap();
    // serial_println!("Before grow: {:?}", memory.data(&mut store).iter().max().unwrap());
    // memory.grow(&mut store, wasmi::core::Pages::new(64).unwrap()).unwrap();
    //serial_println!("After grow: {}", memory.data(&mut store).len());

    serial_println!("Before calling: {}", store.data());

    // And finally we can call the wasm!
    let res = hello.call(&mut store, ()).unwrap();

    let res = res as usize;
    let buffer = memory.data_mut(&mut store);
    serial_println!("Wasm memory: {}", core::str::from_utf8(&buffer[res..res+4]).unwrap());

    

    serial_println!("After calling: {}", store.data());

    serial_println!("Result: {}", res);

    Ok(())
}