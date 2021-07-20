/* Pyexpat builtin module
*
*
*/

use crate::vm::VirtualMachine;
use crate::PyObjectRef;

pub fn make_module(vm: &VirtualMachine) -> PyObjectRef {
    let module = _pyexpat::make_module(vm);

    extend_module!(vm, module, {
         "errors" => _errors::make_module(vm),
         "model" => _model::make_module(vm),
    });

    module
}

macro_rules! create_property {
    ($ctx: expr, $attributes: expr, $name: expr, $element: ident) => {
        let attr = $ctx.new_getset(
            $name,
            move |this: &PyExpatLikeXmlParser| this.$element.read().clone(),
            move |this: &PyExpatLikeXmlParser, func: PyObjectRef| *this.$element.write() = func,
        );

        $attributes.insert($name.to_owned(), attr);
    };
}

#[pymodule(name = "pyexpat")]
mod _pyexpat {
    use crate::builtins::{PyStr, PyStrRef, PyTypeRef};
    use crate::byteslike::PyBytesLike;
    use crate::function::{IntoFuncArgs, OptionalArg};
    use crate::pyobject::StaticType;
    use crate::{
        ItemProtocol, PyContext, PyObjectRef, PyRef, PyResult, PyValue, TryFromObject,
        VirtualMachine,
    };

    use rustpython_common::lock::PyRwLock;
    use std::io::Cursor;
    use xml::reader::XmlEvent;
    type MutableObject = PyRwLock<PyObjectRef>;

    #[pyattr]
    #[pyclass(name = "xmlparser", module = false)]
    #[derive(Debug)]
    pub struct PyExpatLikeXmlParser {
        start_element: MutableObject,
        end_element: MutableObject,
        character_data: MutableObject,
        entity_decl: MutableObject,
        buffer_text: MutableObject,
    }
    type PyExpatLikeXmlParserRef = PyRef<PyExpatLikeXmlParser>;

    impl PyValue for PyExpatLikeXmlParser {
        fn class(_vm: &VirtualMachine) -> &PyTypeRef {
            Self::static_type()
        }
    }

    #[inline]
    fn invoke_handler<T>(vm: &VirtualMachine, handler: &MutableObject, args: T)
    where
        T: IntoFuncArgs,
    {
        vm.invoke(&handler.read().clone(), args).ok();
    }

    #[pyimpl]
    impl PyExpatLikeXmlParser {
        fn new(vm: &VirtualMachine) -> PyResult<PyExpatLikeXmlParserRef> {
            Ok(PyExpatLikeXmlParser {
                start_element: MutableObject::new(vm.ctx.none()),
                end_element: MutableObject::new(vm.ctx.none()),
                character_data: MutableObject::new(vm.ctx.none()),
                entity_decl: MutableObject::new(vm.ctx.none()),
                buffer_text: MutableObject::new(vm.ctx.new_bool(false)),
            }
            .into_ref(vm))
        }

        #[extend_class]
        fn extend_class_with_fields(ctx: &PyContext, class: &PyTypeRef) {
            let mut attributes = class.attributes.write();

            create_property!(ctx, attributes, "StartElementHandler", start_element);
            create_property!(ctx, attributes, "EndElementHandler", end_element);
            create_property!(ctx, attributes, "CharacterDataHandler", character_data);
            create_property!(ctx, attributes, "EntityDeclHandler", entity_decl);
            create_property!(ctx, attributes, "buffer_text", buffer_text);
        }

        fn create_config(&self) -> xml::ParserConfig {
            xml::ParserConfig::new()
                .cdata_to_characters(true)
                .coalesce_characters(false)
                .whitespace_to_characters(true)
        }

        fn do_parse<T>(&self, vm: &VirtualMachine, parser: xml::EventReader<T>)
        where
            T: std::io::Read,
        {
            for e in parser {
                match e {
                    Ok(XmlEvent::StartElement {
                        name, attributes, ..
                    }) => {
                        let dict = vm.ctx.new_dict();
                        for attribute in attributes {
                            dict.set_item(
                                attribute.name.local_name.as_str(),
                                vm.ctx.new_str(attribute.value),
                                vm,
                            )
                            .unwrap();
                        }

                        let name_str = PyStr::from(name.local_name).into_ref(vm);
                        invoke_handler(vm, &self.start_element, (name_str, dict.into_object()));
                    }
                    Ok(XmlEvent::EndElement { name, .. }) => {
                        let name_str = PyStr::from(name.local_name).into_ref(vm);
                        invoke_handler(vm, &self.end_element, (name_str,));
                    }
                    Ok(XmlEvent::Characters(chars)) => {
                        let str = PyStr::from(chars).into_ref(vm);
                        invoke_handler(vm, &self.character_data, (str,));
                    }
                    _ => {}
                }
            }
        }

        #[pymethod(name = "Parse")]
        fn parse(&self, data: PyStrRef, _isfinal: OptionalArg<bool>, vm: &VirtualMachine) {
            let reader = Cursor::<Vec<u8>>::new(data.as_str().as_bytes().to_vec());
            let parser = self.create_config().create_reader(reader);
            self.do_parse(vm, parser);
        }

        #[pymethod(name = "ParseFile")]
        fn parse_file(&self, file: PyObjectRef, vm: &VirtualMachine) -> PyResult<()> {
            // todo: read chunks at a time
            let read_res = vm.call_method(&file, "read", ())?;
            let bytes_like = PyBytesLike::try_from_object(vm, read_res)?;
            let buf = bytes_like.borrow_buf().to_vec();
            let reader = Cursor::new(buf);
            let parser = self.create_config().create_reader(reader);
            self.do_parse(vm, parser);

            // todo: return value
            Ok(())
        }
    }

    #[derive(FromArgs)]
    #[allow(dead_code)]
    struct ParserCreateArgs {
        #[pyarg(any, optional)]
        encoding: OptionalArg<PyStrRef>,
        #[pyarg(any, optional)]
        namespace_separator: OptionalArg<PyStrRef>,
        #[pyarg(any, optional)]
        intern: OptionalArg<PyStrRef>,
    }

    #[pyfunction(name = "ParserCreate")]
    fn parser_create(
        _args: ParserCreateArgs,
        vm: &VirtualMachine,
    ) -> PyResult<PyExpatLikeXmlParserRef> {
        PyExpatLikeXmlParser::new(vm)
    }
}

#[pymodule(name = "model")]
mod _model {}

#[pymodule(name = "errors")]
mod _errors {}
