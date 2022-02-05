use glib::subclass::prelude::*;
use gobject_impl::{interface_impl, object_impl};
use std::cell::Cell;

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
                #[property(get, set, explicit_notify)]
                my_explicit_prop: u64,
            }
        }
        #[signal]
        fn my_sig(&self, hello: i32) {}
    }
    unsafe impl<T: ObjectSubclass> IsImplementable<T> for Dummy {}
    glib::wrapper! {
        pub struct Implementor(ObjectSubclass<ImplementorPrivate>);
    }
    impl Default for Implementor {
        fn default() -> Self {
            glib::Object::new(&[]).unwrap()
        }
    }
    #[glib::object_subclass]
    impl ObjectSubclass for ImplementorPrivate {
        const NAME: &'static str = "Implementor";
        type Type = Implementor;
        type Interfaces = (Dummy,);
    }
    #[object_impl(trait = ImplementorExt)]
    impl ObjectImpl for ImplementorPrivate {
        properties! {
            #[derive(Default)]
            pub struct ImplementorPrivate {
                #[property(get, set, override = Dummy)]
                my_prop: Cell<u64>,
                #[property(get, set, override = Dummy, explicit_notify)]
                my_explicit_prop: Cell<u64>,
            }
        }
    }
}
