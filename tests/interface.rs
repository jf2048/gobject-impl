use glib::subclass::prelude::*;

#[test]
fn interface() {
    glib::wrapper! {
        pub struct Dummy(ObjectInterface<DummyInterface>);
    }
    #[derive(Clone, Copy)]
    pub struct DummyInterface {
        _parent: glib::gobject_ffi::GTypeInterface,
    }
    #[gobject_impl::interface_impl(type = Dummy)]
    #[glib::object_interface]
    unsafe impl ObjectInterface for DummyInterface {
        const NAME: &'static str = "Dummy";
        #[property(get, set)]
        pub my_prop: std::cell::Cell<u64>,
    }
}
