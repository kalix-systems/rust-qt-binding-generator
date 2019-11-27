//! `rust` is the module that generates the rust code for the binding

use crate::configuration::*;
use crate::util::{snake_case, write_if_different};
use std::io::Result;

mod c_ffi;
mod emitter;
mod model;
mod trait_;
mod util;
mod write;

use emitter::*;
use model::*;
use trait_::*;
use util::*;
use write::*;

pub fn write_interface(conf: &Config) -> Result<()> {
    let mut scope = codegen::Scope::new();

    scope.raw("/* generated by riqtshaw */");

    scope.import("riqtshaw_types", "*");
    scope.import(&format!("crate::{}", conf.rust.implementation_module), "*");

    let interface_mod_base = conf
        .out_dir
        .join(&conf.rust.dir)
        .join("src")
        .join(&conf.rust.interface_module);

    for object in conf.objects.values() {
        let module = rust_interface_module(object);

        let mod_name = snake_case(&object.name);
        scope.import(&mod_name, "*").vis("pub");

        scope.raw(&format!("mod {};", mod_name));

        let mut file = interface_mod_base.join(mod_name);
        file.set_extension("rs");

        let mut s_buf = String::new();
        let mut formatter = codegen::Formatter::new(&mut s_buf);
        module.fmt(&mut formatter).unwrap();

        write_if_different(file, s_buf.as_bytes())?;
    }

    let mut file = interface_mod_base.join("mod");
    file.set_extension("rs");

    write_if_different(file, scope.to_string().as_bytes())
}
