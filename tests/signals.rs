use glib::subclass::prelude::*;
use gobject_impl::object_impl;
use std::cell::RefCell;

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
    wrapper!(Signals(SignalsPrivate));
    #[derive(Default)]
    pub struct SignalsPrivate {
        log: RefCell<Vec<String>>,
    }
    impl SignalsPrivate {
        fn append(&self, msg: &str) {
            self.log.borrow_mut().push(msg.to_owned());
        }
    }
    #[object_impl(trait = SignalsExt)]
    impl ObjectImpl for SignalsPrivate {
        #[signal]
        fn noparam(&self) {}
        #[signal]
        fn param(&self, hello: i32) {}
        #[signal]
        fn twoparams(&self, hello: i32, world: String) {}
        #[signal(run_last)]
        fn with_handler(&self, _hello: i32, world: String) {
            self.append(&(world + " last"));
        }
    }
    let signals = Signals::default();

    signals.emit_noparam();
    signals.connect_noparam(|sig| {
        sig.imp().append("noparam");
    });
    signals.emit_noparam();

    signals.connect_with_handler(|sig, hello, world| {
        assert_eq!(hello, 500);
        sig.imp().append(&world);
    });
    signals.emit_with_handler(500, "handler".into());

    assert_eq!(
        *signals.imp().log.borrow(),
        &["noparam", "handler", "handler last"]
    );
}
