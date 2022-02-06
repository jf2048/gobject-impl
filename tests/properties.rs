use glib::prelude::*;
use glib::subclass::prelude::*;
use gobject_impl::object_impl;
use std::cell::{Cell, RefCell};
use std::sync::{Mutex, RwLock};

macro_rules! wrapper {
    ($name:ident($priv:ident)) => {
        glib::wrapper! {
            pub struct $name(ObjectSubclass<$priv>);
        }
        impl Default for $name {
            fn default() -> Self {
                glib::Object::new(&[]).unwrap()
            }
        }
        #[glib::object_subclass]
        impl ObjectSubclass for $priv {
            const NAME: &'static str = stringify!($name);
            type Type = $name;
        }
    };
}

#[test]
fn props() {
    #[derive(Default)]
    struct BasicPropsInner {
        my_bool: Cell<bool>,
    }

    wrapper!(BasicProps(BasicPropsPrivate));
    #[object_impl(trait = BasicPropsExt)]
    impl ObjectImpl for BasicPropsPrivate {
        properties! {
            #[derive(Default)]
            pub struct BasicPropsPrivate {
                #[property(get, set)]
                my_i32: Cell<i32>,
                #[property(get, set)]
                my_str: RefCell<String>,
                #[property(get, set)]
                my_mutex: Mutex<i32>,
                #[property(get, set)]
                my_rw_lock: RwLock<String>,
                #[property(get, set, construct,
                           name = "my-u8", nick = "My U8", blurb = "A uint8",
                           minimum = 5, maximum = 20, default = 19)]
                my_attributed: Cell<u8>,
                #[property(get, set, construct_only, default = 100.0)]
                my_construct_only: Cell<f64>,
                #[property(get, set, explicit_notify)]
                my_explicit: Cell<u64>,
                #[property(get = _, set = _, explicit_notify)]
                my_custom_accessors: RefCell<String>,
                #[property(virtual, get, set, explicit_notify)]
                my_computed_prop: i32,
                #[property(get, set, storage = inner.my_bool)]
                my_delegate: Cell<bool>,
                #[property(get, set, !notify_func, !connect_notify_func)]
                my_no_defaults: Cell<u64>,

                inner: BasicPropsInner
            }
        }
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            obj.connect_my_i32_notify(|obj| obj.notify_my_computed_prop());
        }
    }

    impl BasicProps {
        fn _my_custom_accessors(&self) -> String {
            self.imp().my_custom_accessors.borrow().clone()
        }
        fn _set_my_custom_accessors(&self, value: String) {
            let old = self.imp().my_custom_accessors.replace(value);
            if old != *self.imp().my_custom_accessors.borrow() {
                self.notify_my_custom_accessors();
            }
        }
        fn _my_computed_prop(&self) -> i32 {
            self.my_i32() + 7
        }
        fn _set_my_computed_prop(&self, value: i32) {
            self.set_my_i32(value - 7);
        }
    }

    let props = BasicProps::default();
    assert_eq!(BasicPropsPrivate::properties().len(), 11);
    assert_eq!(props.list_properties().len(), 11);
    props.connect_my_i32_notify(|props| props.set_my_str("Updated".into()));
    assert_eq!(props.my_str(), "");
    props.set_my_i32(5);
    assert_eq!(props.my_i32(), 5);
    assert_eq!(props.property::<i32>("my-i32"), 5);
    assert_eq!(props.my_str(), "Updated");
    assert_eq!(props.property::<String>("my-str"), "Updated");
    assert_eq!(props.my_u8(), 19);
    assert_eq!(props.my_construct_only(), 100.0);
}
