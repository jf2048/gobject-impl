use glib::subclass::prelude::*;
use gobject_impl::object_impl;
use std::cell::RefCell;

#[test]
fn signals() {
    glib::wrapper! {
        pub struct Signals(ObjectSubclass<SignalsPrivate>);
    }
    #[glib::object_subclass]
    impl ObjectSubclass for SignalsPrivate {
        const NAME: &'static str = "Signals";
        type Type = Signals;
    }
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
    let signals = glib::Object::new::<Signals>(&[]).unwrap();

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
