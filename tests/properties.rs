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
    wrapper!(Empty(EmptyPrivate));
    #[derive(Default)]
    pub struct EmptyPrivate {}
    #[object_impl(trait = EmptyExt)]
    impl ObjectImpl for EmptyPrivate {}
    let _ = Empty::default();

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
            }
        }
    }

    let props = BasicProps::default();
    assert_eq!(BasicPropsPrivate::properties().len(), 4);
    assert_eq!(props.list_properties().len(), 4);
    props.connect_my_i32_notify(|props| props.set_my_str("Updated".into()));
    assert_eq!(props.my_str(), "");
    props.set_my_i32(5);
    assert_eq!(props.my_i32(), 5);
    assert_eq!(props.property::<i32>("my-i32"), 5);
    assert_eq!(props.my_str(), "Updated");
    assert_eq!(props.property::<String>("my-str"), "Updated");
}
