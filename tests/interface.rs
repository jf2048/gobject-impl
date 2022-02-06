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
    unsafe impl glib::subclass::interface::ObjectInterface for DummyInterface {
        const NAME: &'static str = "Dummy";
        properties! {
            struct DummyInterface {
                #[property(get, set)]
                my_prop: u64,
                #[property(get, set = auto)]
                my_auto_prop: i64,
            }
        }
        #[signal]
        fn my_sig(&self, hello: i32) {}
    }
    unsafe impl<T: glib::subclass::types::ObjectSubclass> glib::subclass::types::IsImplementable<T>
        for Dummy
    {
    }

    glib::wrapper! {
        pub struct Other(ObjectInterface<OtherInterface>);
    }
    #[derive(Clone, Copy)]
    pub struct OtherInterface {
        _parent: glib::gobject_ffi::GTypeInterface,
    }
    #[interface_impl(type = Other, trait = OtherExt)]
    #[glib::object_interface]
    unsafe impl glib::subclass::interface::ObjectInterface for OtherInterface {
        const NAME: &'static str = "Other";
        properties! {
            struct OtherInterface {
                #[property(get, set)]
                other_prop: i8,
            }
        }
        #[signal]
        fn my_sig(&self, hello: i32) {}
    }

    glib::wrapper! {
        pub struct Implementor(ObjectSubclass<ImplementorPrivate>)
            @implements Dummy;
    }
    impl Default for Implementor {
        fn default() -> Self {
            glib::Object::new(&[]).unwrap()
        }
    }
    #[glib::object_subclass]
    impl glib::subclass::types::ObjectSubclass for ImplementorPrivate {
        const NAME: &'static str = "Implementor";
        type Type = Implementor;
        type Interfaces = (Dummy,);
    }
    #[object_impl(trait = ImplementorExt)]
    impl glib::subclass::object::ObjectImpl for ImplementorPrivate {
        properties! {
            #[derive(Default)]
            pub struct ImplementorPrivate {
                #[property(get, set, override = Dummy)]
                my_prop: Cell<u64>,
                #[property(get, set = auto, override = Dummy, minimum = -10, maximum = 10)]
                my_auto_prop: Cell<i64>,
                #[property(get, set, override = Other, !inherit)]
                other_prop: Cell<i8>,
            }
        }
    }

    let obj = glib::Object::new::<Implementor>(&[]).unwrap();
    obj.set_my_prop(4000);
    obj.set_my_auto_prop(-5);
    obj.set_other_prop(22);
}
