use rustpython::vm::{self as vm, AsObject};
use rustpython::vm::{Interpreter, scope::Scope};

pub struct Python {
    interpreter: Interpreter,
    scope_save: Option<Scope>
}

pub enum EvalResult {
    None,
    Success(String),
    Failure(String),
}

fn test_func(s: String) {
    println!("From Python: {}", s);
}

impl Python {

    pub fn new() -> Self {

        let interpreter = rustpython::InterpreterConfig::new()
            .init_stdlib()
            .interpreter();

        Python {
            interpreter,
            scope_save: None
        }
    }

    pub fn run_code(&mut self, source: &str) -> EvalResult {

        self.interpreter.enter(|vm| {

            let res = || -> vm::PyResult<Option<String>> {

                let scope = match self.scope_save.as_ref() {
                    Some(scope) => scope.clone(),
                    None => {
                        let scope = vm.new_scope_with_builtins();
                        scope
                            .globals
                            .set_item("test_func", vm.new_function("test_func", test_func).into(), vm)?;
                        let _ = self.scope_save.insert(scope.clone());
                        scope
                    }
                };
        
                let code_obj = vm
                    .compile(source, vm::compiler::Mode::BlockExpr, "<embedded>".to_owned())
                    .map_err(|err| vm.new_syntax_error(&err, Some(source)))?;
        
                let obj = vm.run_code_obj(code_obj, scope)?;

                let repr = match vm.is_none(obj.as_object()) {
                    true => None,
                    false => Some(obj.repr(vm)?.as_str().to_owned())
                };

                Ok(repr)

            }();

            match res {
                Ok(Some(repr)) => EvalResult::Success(repr),
                Ok(None) => EvalResult::None,
                Err(err) => {
                    let mut exc_s = String::new();
                    vm.write_exception(&mut exc_s, &err).unwrap();
                    EvalResult::Failure(exc_s)
                },
            }
        })
    }

}