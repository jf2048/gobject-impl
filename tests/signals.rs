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
        #[signal(run_first)]
        fn with_retval(&self, val: i32) -> i32 {
            val + 5
        }
        #[signal(run_first)]
        fn with_accumulator(&self, val: i32) -> i32 {
            val + 10
        }
        #[accumulator]
        fn with_accumulator(accu: &mut i32, val: i32) -> glib::Continue {
            *accu += val;
            glib::Continue(true)
        }
        #[signal(detailed, run_cleanup)]
        fn has_detail(&self, val: u32) -> u32 {
            val + 7
        }
        #[accumulator]
        fn has_detail(
            hint: &glib::subclass::signal::SignalInvocationHint,
            accu: &mut u32,
            val: u32,
        ) -> glib::Continue {
            if let Some(quark) = hint.detail() {
                if quark.as_str() == "hello" {
                    *accu += 100;
                }
            }
            *accu += val;
            glib::Continue(true)
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

    assert_eq!(signals.emit_with_retval(10), 15);
    signals.connect_with_retval(|_, val| val * 2);
    assert_eq!(signals.emit_with_retval(10), 20);

    assert_eq!(signals.emit_with_accumulator(10), 20);
    signals.connect_with_accumulator(|_, val| val * 3);
    assert_eq!(signals.emit_with_accumulator(10), 50);

    assert_eq!(signals.emit_has_detail(None, 10), 17);
    assert_eq!(signals.emit_has_detail(Some("hello".into()), 10), 117);
    signals.connect_has_detail(Some("hello".into()), |_, val| val * 3);
    assert_eq!(signals.emit_has_detail(None, 20), 27);
    assert_eq!(signals.emit_has_detail(Some("hello".into()), 20), 287);
}
