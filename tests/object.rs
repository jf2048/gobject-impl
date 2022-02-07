mod obj_final {
    glib::wrapper! {
        pub struct ObjFinal(ObjectSubclass<imp::ObjFinal>);
    }
    mod imp {
        #[glib::object_subclass]
        impl glib::subclass::types::ObjectSubclass for ObjFinal {
            const NAME: &'static str = "ObjFinal";
            type Type = super::ObjFinal;
        }
        #[gobject_impl::object_impl(final, type = super::ObjFinal)]
        impl glib::subclass::object::ObjectImpl for ObjFinal {
            properties! {
                #[derive(Default)]
                pub struct ObjFinal {
                    #[property(get, set)]
                    my_prop: std::cell::Cell<u64>,
                }
            }
            #[signal]
            fn abc(&self) {}
        }
    }
}

#[test]
fn object_final() {
    let obj = glib::Object::new::<obj_final::ObjFinal>(&[]).unwrap();
    obj.set_my_prop(52);
    obj.emit_abc();
}

mod obj_abstract {
    pub use imp::ObjAbstractExt;
    glib::wrapper! {
        pub struct ObjAbstract(ObjectSubclass<imp::ObjAbstract>);
    }
    mod imp {
        #[glib::object_subclass]
        impl glib::subclass::types::ObjectSubclass for ObjAbstract {
            const NAME: &'static str = "ObjAbstract";
            type Type = super::ObjAbstract;
        }
        #[gobject_impl::object_impl(trait = ObjAbstractExt)]
        impl glib::subclass::object::ObjectImpl for ObjAbstract {
            properties! {
                #[derive(Default)]
                pub struct ObjAbstract {
                    #[property(get, set)]
                    my_prop: std::cell::Cell<u64>,
                }
            }
            #[signal]
            fn abc(&self) {}
        }
    }
}

#[test]
fn object_abstract() {
    pub use obj_abstract::ObjAbstractExt;
    let obj = glib::Object::new::<obj_abstract::ObjAbstract>(&[]).unwrap();
    obj.set_my_prop(52);
    obj.emit_abc();
}

mod obj_inner {
    pub use imp::ObjInnerExt;
    glib::wrapper! {
        pub struct ObjInner(ObjectSubclass<imp::ObjInner>);
    }
    mod imp {
        #[glib::object_subclass]
        impl glib::subclass::types::ObjectSubclass for ObjInner {
            const NAME: &'static str = "ObjInner";
            type Type = super::ObjInner;
        }
        #[gobject_impl::object_impl(trait = ObjInnerExt)]
        impl glib::subclass::object::ObjectImpl for ObjInner {
            properties! {
                #[derive(Default)]
                pub struct ObjInner {
                    #[property(get, set)]
                    my_prop: std::cell::Cell<u64>,
                }
            }
            #[signal]
            fn abc(&self) {}
            fn properties() -> &'static [glib::ParamSpec] {
                use glib::once_cell::sync::Lazy as SyncLazy;
                static PROPERTIES: SyncLazy<Vec<glib::ParamSpec>> = SyncLazy::new(|| {
                    let mut props = ObjInner::inner_properties().to_owned();
                    props.push(glib::ParamSpecUInt::new(
                        "my-uint",
                        "my-uint",
                        "my-uint",
                        0,
                        0,
                        0,
                        glib::ParamFlags::READWRITE,
                    ));
                    props
                });
                PROPERTIES.as_ref()
            }
            fn signals() -> &'static [glib::subclass::Signal] {
                use glib::once_cell::sync::Lazy as SyncLazy;
                static SIGNALS: SyncLazy<Vec<glib::subclass::Signal>> = SyncLazy::new(|| {
                    let mut signals = ObjInner::inner_signals();
                    signals.push(
                        glib::subclass::Signal::builder("xyz", &[], glib::Type::UNIT.into())
                            .build(),
                    );
                    signals
                });
                SIGNALS.as_ref()
            }
        }
    }
}

#[test]
fn object_inner_methods() {
    use glib::prelude::*;
    use obj_inner::ObjInnerExt;

    let obj = glib::Object::new::<obj_inner::ObjInner>(&[]).unwrap();
    assert_eq!(obj.list_properties().len(), 2);
    obj.emit_abc();
    obj.emit_by_name::<()>("xyz", &[]);
}
