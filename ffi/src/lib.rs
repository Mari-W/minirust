mod translate;
use crate::translate::ToRustCode;
use ast::ctx::Ctx;
use ast::err::{Error, Result};
use ast::{Debruijn, Type, Value, _Ident, _Program, _Type, _Value, _Vec};
use libloading::Library;
use rand::{distributions::Alphanumeric, Rng};
use std::fs;
use std::path::Path;
use std::process::Command;

pub struct FFI {
    module_path: String,
    code_path: String,
    lib: Library,
}

impl FFI {
    pub fn new(program: &_Program<Debruijn>, env: &Ctx<_Type<Debruijn>>) -> Result<Self> {
        let tmp = std::env::temp_dir().to_str().unwrap().to_string();
        let id: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(24)
            .map(char::from)
            .collect();
        let module_path = format!("{}/{}.module", tmp, id);
        let code_path = format!("{}/{}.rs", tmp, id);

        fs::write(&code_path, program.to_rust(env)?)
            .map_err(|e| Error::new(format!("failed to write module {}", e)))?;

        if !Command::new("rustc")
            .arg("--crate-type")
            .arg("dylib")
            .arg("-A")
            .arg("warnings")
            .arg("-o")
            .arg(&module_path)
            .arg(&code_path)
            .status()
            .map_err(|e| {
                Error::new(format!("failed to compile {}", e)).help("rustc needs to be installed")
            })?
            .success()
        {
            return Err(Error::new(format!(
                "rust error in {}.foo",
                program.tag.0.join("/")
            )));
        }

        Ok(FFI {
            module_path,
            code_path,
            lib: unsafe {
                Library::new(&format!("{}/{}.module", tmp, id))
                    .map_err(|e| Error::new(format!("failed to load module {}", e)))?
            },
        })
    }

    fn _call(
        &self,
        function: &_Ident<Debruijn>,
        args: &_Vec<Debruijn, (_Value<Debruijn>, _Type<Debruijn>)>,
        ty: &_Type<Debruijn>,
    ) -> std::result::Result<_Value<Debruijn>, libloading::Error> {
        Ok(function.set(match (args.it().len(), ty.it()) {
            (0, Type::Unit) => {
                self.call0::<()>(function.it())?;
                Value::Unit
            }
            (0, Type::Bool) => Value::Bool(self.call0(function.it())?),
            (0, Type::Int) => Value::Int(self.call0(function.it())?),
            (0, Type::Str) => Value::Str(self.call0(function.it())?),
            (1, Type::Unit) => {
                call1!(self, args, function);
                Value::Unit
            }
            (1, Type::Bool) => Value::Bool(call1!(self, args, function)),
            (1, Type::Int) => Value::Int(call1!(self, args, function)),
            (1, Type::Str) => Value::Str(call1!(self, args, function)),
            (2, Type::Unit) => {
                call2!(self, args, function);
                Value::Unit
            }
            (2, Type::Bool) => Value::Bool(call2!(self, args, function)),
            (2, Type::Int) => Value::Int(call2!(self, args, function)),
            (2, Type::Str) => Value::Str(call2!(self, args, function)),
            _ => unimplemented!(),
        }))
    }

    pub fn call(
        &self,
        function: &_Ident<Debruijn>,
        args: &_Vec<Debruijn, (_Value<Debruijn>, _Type<Debruijn>)>,
        ty: &_Type<Debruijn>,
    ) -> Result<_Value<Debruijn>> {
        self._call(function, args, ty).map_err(|e| {
            Error::new("call to ffi function failed with")
                .label(function, format!("failed with {}", e))
        })
    }
    fn call0<R>(&self, function: &String) -> std::result::Result<R, libloading::Error> {
        unsafe { Ok((self.lib.get::<unsafe fn() -> R>(function.as_bytes())?)()) }
    }
    fn call1<T1, R>(&self, function: &String, t1: T1) -> std::result::Result<R, libloading::Error> {
        unsafe {
            Ok((self.lib.get::<unsafe fn(T1) -> R>(function.as_bytes())?)(
                t1,
            ))
        }
    }
    fn call2<T1, T2, R>(
        &self,
        function: &String,
        t1: T1,
        t2: T2,
    ) -> std::result::Result<R, libloading::Error> {
        unsafe {
            Ok((self
                .lib
                .get::<unsafe fn(T1, T2) -> R>(function.as_bytes())?)(
                t1, t2,
            ))
        }
    }
}
impl Drop for FFI {
    fn drop(&mut self) {
        let src = Path::new(&self.code_path);
        if src.exists() {
            fs::remove_file(src).unwrap()
        }
        let lib = Path::new(&self.module_path);
        if lib.exists() {
            fs::remove_file(lib).unwrap()
        }
    }
}

#[macro_export]
macro_rules! call1 {
    ($self:ident, $args:ident, $function:ident) => {
        match $args.it()[0].0.it() {
            Value::Unit => $self.call1($function.it(), ())?,
            Value::Bool(b) => $self.call1($function.it(), b.clone())?,
            Value::Int(i) => $self.call1($function.it(), i.clone())?,
            Value::Str(s) => $self.call1($function.it(), s.clone())?,
            _ => unimplemented!(),
        }
    };
}

#[macro_export]
macro_rules! call2 {
    ($self:ident, $args:ident, $function:ident) => {
        match ($args.it()[0].0.it(), $args.it()[1].0.it()) {
            (Value::Unit, Value::Unit) => $self.call2($function.it(), (), ())?,
            (Value::Unit, Value::Bool(b)) => $self.call2($function.it(), (), b)?,
            (Value::Unit, Value::Int(i)) => $self.call2($function.it(), (), i)?,
            (Value::Unit, Value::Str(s)) => $self.call2($function.it(), (), s)?,
            (Value::Bool(b), Value::Unit) => $self.call2($function.it(), b, ())?,
            (Value::Bool(b), Value::Bool(b2)) => $self.call2($function.it(), b, b2)?,
            (Value::Bool(b), Value::Int(i)) => $self.call2($function.it(), b, i)?,
            (Value::Bool(b), Value::Str(s)) => $self.call2($function.it(), b, s)?,
            (Value::Int(i), Value::Unit) => $self.call2($function.it(), i, ())?,
            (Value::Int(i), Value::Bool(b)) => $self.call2($function.it(), i, b)?,
            (Value::Int(i), Value::Int(i2)) => $self.call2($function.it(), i, i2)?,
            (Value::Int(i), Value::Str(s)) => $self.call2($function.it(), i, s)?,
            (Value::Str(s), Value::Unit) => $self.call2($function.it(), s, ())?,
            (Value::Str(s), Value::Bool(b)) => $self.call2($function.it(), s, b)?,
            (Value::Str(s), Value::Int(i)) => $self.call2($function.it(), s, i)?,
            (Value::Str(s), Value::Str(s2)) => $self.call2($function.it(), s, s2)?,
            _ => unimplemented!(),
        }
    };
}
