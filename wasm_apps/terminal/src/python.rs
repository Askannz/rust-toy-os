use rustpython::vm as vm;
use rustpython::vm::{Interpreter, scope::Scope};

pub struct Python {
    interpreter: Interpreter,
    scope_save: Option<Scope>
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

    pub fn run_code(&mut self, source: &str) -> String {

        let res: vm::PyResult<String> = self.interpreter.enter(|vm| {

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
    
            let obj = vm.
                run_code_obj(code_obj, scope)
                .inspect_err(|err| {
                    let mut exc_s = String::new();
                    vm.write_exception(&mut exc_s, err).unwrap();
                    println!("{}", exc_s)
                })?;
    
    
            let repr = obj.repr(vm)?;
    
            println!("Python result: {}", repr.as_str());
    
            //scope_save.insert(scope);
    
            Ok(repr.as_str().to_owned())
        });
    
        res.unwrap() 
    }

}