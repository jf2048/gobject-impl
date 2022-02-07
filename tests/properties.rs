use glib::once_cell::unsync::OnceCell;
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
        #[glib::object_subclass]
        impl ObjectSubclass for $priv {
            const NAME: &'static str = stringify!($name);
            type Type = $name;
        }
    };
}

#[test]
fn basic_properties() {
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
                #[property(get)]
                readable_i32: Cell<i32>,
                #[property(set)]
                writable_i32: Cell<i32>,
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
                #[property(get, set, explicit_notify, lax_validation)]
                my_explicit: Cell<u64>,
                #[property(get, set = auto, construct_only)]
                my_auto_set: OnceCell<f32>,
                #[property(get = _, set = _, explicit_notify, lax_validation)]
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
        fn my_custom_accessors(&self) -> String {
            self.imp().my_custom_accessors.borrow().clone()
        }
        fn set_my_custom_accessors(&self, value: String) {
            let old = self.imp().my_custom_accessors.replace(value);
            if old != *self.imp().my_custom_accessors.borrow() {
                self.notify_my_custom_accessors();
            }
        }
        fn my_computed_prop(&self) -> i32 {
            self.my_i32() + 7
        }
        fn _set_my_computed_prop(&self, value: i32) {
            self.set_my_i32(value - 7);
        }
    }

    let props = glib::Object::new::<BasicProps>(&[]).unwrap();
    assert_eq!(BasicPropsPrivate::properties().len(), 14);
    assert_eq!(props.list_properties().len(), 14);
    props.connect_my_i32_notify(|props| props.set_my_str("Updated".into()));
    assert_eq!(*props.my_str(), "");
    props.set_my_i32(5);
    assert_eq!(props.my_i32(), 5);
    assert_eq!(props.property::<i32>("my-i32"), 5);
    assert_eq!(*props.my_str(), "Updated");
    assert_eq!(props.property::<String>("my-str"), "Updated");
    assert_eq!(props.my_u8(), 19);
    assert_eq!(props.my_construct_only(), 100.0);
}

#[test]
fn complex_properties() {
    use glib::once_cell::unsync::OnceCell;

    wrapper!(DummyObject(DummyObjectPrivate));
    #[object_impl(trait = DummyObjectExt)]
    impl ObjectImpl for DummyObjectPrivate {
        properties! {
            #[derive(Default)]
            pub struct DummyObjectPrivate {
                #[property(name = "renamed-string", get, set, construct, default = "foobar")]
                a_string: RefCell<String>,
            }
        }
    }

    #[derive(Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
    #[repr(u32)]
    #[enum_type(name = "TestAnimalType")]
    pub enum Animal {
        Goat,
        Dog,
        Cat,
        Badger,
    }

    wrapper!(ComplexProps(ComplexPropsPrivate));
    impl Default for ComplexPropsPrivate {
        fn default() -> Self {
            Self {
                object_type: Cell::new(glib::Object::static_type()),
                time: RefCell::new(glib::DateTime::from_utc(1970, 1, 1, 0, 0, 0.).unwrap()),
                optional_time: Default::default(),
                dummy: Default::default(),
                animal: Cell::new(Animal::Dog),
                binding_flags: Cell::new(glib::BindingFlags::empty()),
                pspec: RefCell::new(<Self as ObjectSubclass>::Type::pspec_dummy().clone()),
                variant: RefCell::new(1i32.to_variant()),
                renamed_string: Default::default(),
            }
        }
    }
    #[object_impl(trait = ComplexPropsExt)]
    impl ObjectImpl for ComplexPropsPrivate {
        properties! {
            pub struct ComplexPropsPrivate {
                #[property(get, set, subtype = glib::Object)]
                object_type: Cell<glib::Type>,
                #[property(get, set, boxed)]
                time: RefCell<glib::DateTime>,
                #[property(get, set, boxed)]
                optional_time: RefCell<Option<glib::DateTime>>,
                #[property(get, set, object, construct_only)]
                dummy: OnceCell<DummyObject>,
                #[property(get, set, enum)]
                animal: Cell<Animal>,
                #[property(get, set, flags)]
                binding_flags: Cell<glib::BindingFlags>,
                #[property(get, set, subtype = glib::ParamSpecObject)]
                pspec: RefCell<glib::ParamSpec>,
                #[property(get, set, variant = "i")]
                variant: RefCell<glib::Variant>,
                #[property(get, set, override_class = DummyObject)]
                renamed_string: RefCell<String>,
            }
        }
    }

    let dummy = glib::Object::new::<DummyObject>(&[]).unwrap();
    let obj = glib::Object::new::<ComplexProps>(&[("dummy", &dummy)]).unwrap();
    obj.set_renamed_string("hello".into());
    assert_eq!(&*obj.dummy().renamed_string(), "foobar");
}

#[test]
fn pod_type() {
    wrapper!(Pod(PodPrivate));
    #[object_impl(final, pod, type = Pod)]
    impl ObjectImpl for PodPrivate {
        properties! {
            #[derive(Default)]
            pub struct PodPrivate {
                int_prop: Cell<i32>,
                string_prop: RefCell<String>,

                #[property(skip)]
                skipped_field: Vec<(i32, bool)>,
            }
        }
    }

    let obj = glib::Object::new::<Pod>(&[]).unwrap();
    assert_eq!(obj.list_properties().len(), 2);
    assert!(obj.imp().skipped_field.is_empty());
    obj.set_int_prop(5);
    obj.set_string_prop("123".into());
}
