use glib::prelude::*;
use glib::subclass::prelude::*;
use gobject_impl::object_impl;
use std::cell::{Cell, RefCell};
use std::sync::{Mutex, RwLock};

macro_rules! wrapper {
    ($name:ident($priv:ident)) => {
        glib::wrapper! {
            struct $name(ObjectSubclass<$priv>);
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
    #[object_impl]
    #[derive(Default)]
    struct EmptyPrivate {}
    let _ = EmptyPrivate::default();

    wrapper!(EmptyTrait(EmptyTraitPrivate));
    #[object_impl(impl_trait)]
    #[derive(Default)]
    struct EmptyTraitPrivate {}
    impl ObjectImpl for EmptyTraitPrivate {}
    let _ = EmptyTrait::default();

    wrapper!(EmptyProps(EmptyPropsPrivate));
    #[object_impl]
    #[derive(Default)]
    struct EmptyPropsPrivate {
        #[property]
        _my_i32: Cell<i32>,
        #[property]
        _my_str: RefCell<String>,
        #[property]
        _my_mutex: Mutex<i32>,
        #[property]
        _my_rw_lock: RwLock<String>,
    }
    let empty = EmptyProps::default();
    assert_eq!(empty.list_properties().len(), 0);

    wrapper!(BasicProps(BasicPropsPrivate));
    #[object_impl]
    #[derive(Default)]
    struct BasicPropsPrivate {
        #[property(get, set)]
        _my_i32: Cell<i32>,
    }

    let props = BasicProps::default();
    assert_eq!(props.list_properties().len(), 1);
    props.set_my_i32(5);
    assert_eq!(props.my_i32(), 5);
    assert_eq!(props.property::<i32>("my-i32"), 5);
}
