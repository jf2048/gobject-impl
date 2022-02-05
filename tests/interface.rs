use glib::subclass::prelude::*;
use gobject_impl::interface_impl;

#[test]
fn interface() {
    glib::wrapper! {
        pub struct Dummy(ObjectInterface<DummyInterface>);
    }
    #[derive(Clone, Copy)]
    pub struct DummyInterface {
        _parent: glib::gobject_ffi::GTypeInterface,
    }
    #[interface_impl(type = Dummy, trait = DummyExt)]
    #[glib::object_interface]
    unsafe impl ObjectInterface for DummyInterface {
        const NAME: &'static str = "Dummy";
        properties! {
            struct DummyInterface {
                #[property(get, set)]
                my_prop: u64,
            }
        }
    }
}
