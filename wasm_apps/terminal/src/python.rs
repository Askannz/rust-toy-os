use std::borrow::{Borrow, BorrowMut};
use std::sync::Mutex;
use alloc::rc::Rc;
use core::cell::RefCell;
use rustpython::vm::{self as vm, AsObject};
use rustpython::vm::{Interpreter, scope::Scope};
use rustpython::vm::function::IntoPyNativeFn;

pub struct Python {
    interpreter: Interpreter,
    scope: Scope,
    console_sink: ConsoleSink,
}

#[derive(Clone)]
struct ConsoleSink {
    pointer: *mut u8,
    length: usize,
    capacity: usize,
}

// YOLO
unsafe impl Send for ConsoleSink {}
unsafe impl Sync for ConsoleSink {}

impl ConsoleSink {
    fn new() -> Self {
        let s = String::new();
        let (pointer, length, capacity) = s.into_raw_parts();
        ConsoleSink { pointer, length, capacity }
    }

    fn push(&self, new_s: &str) {
        let ConsoleSink { pointer, length, capacity } = self;
        let mut s = unsafe { String::from_raw_parts(pointer.as_mut().unwrap(), *length, *capacity) };
        s.push_str(new_s);
    }

    fn clear(&self) {
        let ConsoleSink { pointer, length, capacity } = self;
        let mut s = unsafe { String::from_raw_parts(pointer.as_mut().unwrap(), *length, *capacity) };
        s.clear();
    }

    fn get_string(&self) -> String {
        let ConsoleSink { pointer, length, capacity } = self;
        let s = unsafe { String::from_raw_parts(pointer.as_mut().unwrap(), *length, *capacity) };
        s.clone()
    }
}


pub enum EvalResult {
    None,
    Success(String),
    Failure(String),
}

const PRELUDE: &'static str = include_str!("prelude.py");
const PRINT_FUNC: &'static str = "__RustPythonHostConsole__rustpython_host_console";

impl Python {

    pub fn new() -> Self {

        let interpreter = rustpython::InterpreterConfig::new()
            .init_stdlib()
            .interpreter();

        let console_sink = ConsoleSink::new();

        let console_sink_c = console_sink.clone();
        let host_print = move |s: String| {
            console_sink_c.push(s.as_str());
        };

        let scope = interpreter.enter(|vm| {
            let scope = vm.new_scope_with_builtins();
            scope
                .globals
                .set_item(PRINT_FUNC, vm.new_function(PRINT_FUNC, host_print).into(), vm)
                .unwrap();
            scope
        });

        let mut python = Python {
            interpreter,
            scope,
            console_sink,
        };

        python.run_code(&PRELUDE);

        python
    }

    pub fn run_code(&mut self, source: &str) -> EvalResult {

        self.console_sink.clear();

        self.interpreter.enter(|vm| {

            let res = || -> vm::PyResult<Option<String>> {
        
                let code_obj = vm
                    .compile(source, vm::compiler::Mode::BlockExpr, "<embedded>".to_owned())
                    .map_err(|err| vm.new_syntax_error(&err, Some(source)))?;
        

                let obj = vm.run_code_obj(code_obj, self.scope.clone())?;

                let repr = match vm.is_none(obj.as_object()) {
                    true => None,
                    false => Some(obj.repr(vm)?.as_str().to_owned())
                };

                Ok(repr)

            }();

            let out_str = self.console_sink.get_string();

            let return_str = match res {
                Ok(Some(repr)) => EvalResult::Success(format!("{}\n{}", out_str, repr)),
                Ok(None) => EvalResult::None,
                Err(err) => {
                    let mut exc_s = String::new();
                    vm.write_exception(&mut exc_s, &err).unwrap();
                    EvalResult::Failure(format!("{}\n{}", out_str, exc_s))
                },
            };

            return_str
        })
    }

}