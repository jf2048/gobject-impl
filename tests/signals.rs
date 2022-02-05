use glib::prelude::*;
use glib::subclass::prelude::*;
use gobject_impl::object_impl;

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
    struct EmptyPrivate {
        #[signal]
        signal abc(&self);
    }
    let _ = EmptyPrivate::default();
}

