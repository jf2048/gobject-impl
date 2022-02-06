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
