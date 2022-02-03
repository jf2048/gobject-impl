use std::ops::DerefMut;

use glib::{translate::*, value::ValueType, ParamFlags, ParamSpec, Value};

pub use gobject_impl_macros::*;

macro_rules! define_defaulted {
    ($ty:ty, $name:ty, $builder_name:ident) => {
        pub struct $builder_name {
            default: $ty,
        }
        impl $builder_name {
            pub fn default(mut self, value: $ty) -> Self {
                self.default = value;
                self
            }
        }
        impl HasParamSpec for $ty {
            type Builder = $builder_name;
            fn builder() -> $builder_name {
                $builder_name {
                    default: Default::default(),
                }
            }
        }
        impl ParamSpecBuilder for $builder_name {
            fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
                <$name>::new(name, nick, blurb, self.default, flags)
            }
        }
    };
}

macro_rules! define_numeric {
    ($ty:ty, $name:ty, $builder_name:ident) => {
        pub struct $builder_name {
            minimum: $ty,
            maximum: $ty,
            default: $ty,
        }
        impl $builder_name {
            pub fn minimum(mut self, value: $ty) -> Self {
                self.minimum = value;
                self
            }
            pub fn maximum(mut self, value: $ty) -> Self {
                self.maximum = value;
                self
            }
            pub fn default(mut self, value: $ty) -> Self {
                self.default = value;
                self
            }
        }
        impl HasParamSpec for $ty {
            type Builder = $builder_name;
            fn builder() -> $builder_name {
                $builder_name {
                    minimum: <$ty>::MIN,
                    maximum: <$ty>::MAX,
                    default: Default::default(),
                }
            }
        }
        impl ParamSpecBuilder for $builder_name {
            fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
                <$name>::new(
                    name,
                    nick,
                    blurb,
                    self.minimum,
                    self.maximum,
                    self.default,
                    flags,
                )
            }
        }
    };
}

pub trait HasParamSpec {
    type Builder;
    fn builder() -> <Self as HasParamSpec>::Builder;
}

pub trait ParamSpecBuilder {
    fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec;
}

define_defaulted!(bool, glib::ParamSpecBoolean, ParamSpecBooleanBuilder);
define_numeric!(i8, glib::ParamSpecChar, ParamSpecCharBuilder);
define_numeric!(u8, glib::ParamSpecUChar, ParamSpecUCharBuilder);
define_numeric!(i32, glib::ParamSpecInt, ParamSpecIntBuilder);
define_numeric!(u32, glib::ParamSpecUInt, ParamSpecUIntBuilder);
define_numeric!(i64, glib::ParamSpecInt64, ParamSpecInt64Builder);
define_numeric!(u64, glib::ParamSpecUInt64, ParamSpecUInt64Builder);
define_numeric!(f32, glib::ParamSpecFloat, ParamSpecFloatBuilder);
define_numeric!(f64, glib::ParamSpecDouble, ParamSpecDoubleBuilder);
define_defaulted!(char, glib::ParamSpecUnichar, ParamSpecUnicharBuilder);

pub struct ParamSpecStringBuilder {
    default: Option<&'static str>,
}
impl ParamSpecStringBuilder {
    pub fn default(mut self, value: &'static str) -> Self {
        self.default = Some(value);
        self
    }
}
impl HasParamSpec for String {
    type Builder = ParamSpecStringBuilder;
    fn builder() -> ParamSpecStringBuilder {
        ParamSpecStringBuilder { default: None }
    }
}
impl ParamSpecBuilder for ParamSpecStringBuilder {
    fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        glib::ParamSpecString::new(name, nick, blurb, self.default, flags)
    }
}
impl HasParamSpec for Option<String> {
    type Builder = ParamSpecStringBuilder;
    fn builder() -> ParamSpecStringBuilder {
        <String as HasParamSpec>::builder()
    }
}

pub struct ParamSpecParamBuilder {
    type_: glib::Type,
}
impl ParamSpecParamBuilder {
    pub fn type_<T: glib::ParamSpecType>(mut self) -> Self {
        self.type_ = T::static_type();
        self
    }
}
impl HasParamSpec for ParamSpec {
    type Builder = ParamSpecParamBuilder;
    fn builder() -> ParamSpecParamBuilder {
        ParamSpecParamBuilder {
            type_: glib::Type::UNIT,
        }
    }
}
impl ParamSpecBuilder for ParamSpecParamBuilder {
    fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        if self.type_ == glib::Type::UNIT {
            panic!(
                "property `{}` must specify a type implementing glib::ParamSpecType",
                name
            );
        }
        glib::ParamSpecParam::new(name, nick, blurb, self.type_, flags)
    }
}
impl HasParamSpec for Option<ParamSpec> {
    type Builder = ParamSpecParamBuilder;
    fn builder() -> ParamSpecParamBuilder {
        <ParamSpec as HasParamSpec>::builder()
    }
}

pub struct ParamSpecPointerBuilder {}
impl<T> HasParamSpec for *mut T {
    type Builder = ParamSpecPointerBuilder;
    fn builder() -> ParamSpecPointerBuilder {
        ParamSpecPointerBuilder {}
    }
}
impl<T> HasParamSpec for *const T {
    type Builder = ParamSpecPointerBuilder;
    fn builder() -> ParamSpecPointerBuilder {
        ParamSpecPointerBuilder {}
    }
}
impl ParamSpecBuilder for ParamSpecPointerBuilder {
    fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        glib::ParamSpecPointer::new(name, nick, blurb, flags)
    }
}

pub struct ParamSpecGTypeBuilder {
    type_: glib::Type,
}
impl ParamSpecGTypeBuilder {
    pub fn type_<T: glib::StaticType>(mut self) -> Self {
        self.type_ = T::static_type();
        self
    }
}
impl HasParamSpec for glib::Type {
    type Builder = ParamSpecGTypeBuilder;
    fn builder() -> ParamSpecGTypeBuilder {
        ParamSpecGTypeBuilder {
            type_: glib::Type::UNIT,
        }
    }
}
impl ParamSpecBuilder for ParamSpecGTypeBuilder {
    fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        glib::ParamSpecGType::new(name, nick, blurb, self.type_, flags)
    }
}

pub struct ParamSpecVariantBuilder {
    type_: &'static glib::VariantTy,
    default: Option<&'static str>,
}
impl ParamSpecVariantBuilder {
    pub fn type_(mut self, type_: &'static str) -> Self {
        self.type_ = glib::VariantTy::new(type_).unwrap();
        self
    }
    pub fn default(mut self, value: &'static str) -> Self {
        self.default = Some(value);
        self
    }
}
impl HasParamSpec for glib::Variant {
    type Builder = ParamSpecVariantBuilder;
    fn builder() -> ParamSpecVariantBuilder {
        ParamSpecVariantBuilder {
            type_: glib::VariantTy::ANY,
            default: None,
        }
    }
}
impl ParamSpecBuilder for ParamSpecVariantBuilder {
    fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        let mut error = std::ptr::null_mut();
        let default = self.default.map(|s| {
            let end = &s[s.len()..];
            unsafe {
                let variant = glib::ffi::g_variant_parse(
                    self.type_.to_glib_none().0,
                    s.as_ptr() as *const _,
                    end.as_ptr() as *const _,
                    std::ptr::null_mut(),
                    &mut error,
                );
                if error.is_null() {
                    from_glib_none(variant)
                } else {
                    let err: glib::Error = from_glib_full(error);
                    panic!("{}", err);
                }
            }
        });
        glib::ParamSpecVariant::new(name, nick, blurb, self.type_, default.as_ref(), flags)
    }
}
impl HasParamSpec for Option<glib::Variant> {
    type Builder = ParamSpecVariantBuilder;
    fn builder() -> ParamSpecVariantBuilder {
        <glib::Variant as HasParamSpec>::builder()
    }
}

pub trait ParamStoreRead<T: ValueType> {
    fn get_value(&self) -> glib::Value;
}
pub trait ParamStoreWrite<T: ValueType> {
    fn set(&self, value: T) -> bool;
    fn set_value(&self, value: &Value) -> bool {
        let v = value.get_owned().expect("Invalid value for property");
        self.set(v)
    }
}

impl<T: HasParamSpec> HasParamSpec for std::cell::Cell<T> {
    type Builder = T::Builder;
    fn builder() -> <Self as HasParamSpec>::Builder {
        T::builder()
    }
}
impl<T: ValueType + Copy + HasParamSpec> ParamStoreRead<T> for std::cell::Cell<T> {
    fn get_value(&self) -> glib::Value {
        std::cell::Cell::get(self).to_value()
    }
}
impl<T: ValueType + HasParamSpec + PartialEq + Copy> ParamStoreWrite<T> for std::cell::Cell<T> {
    fn set(&self, value: T) -> bool {
        let old = self.replace(value);
        old != self.get()
    }
}

impl<T: HasParamSpec> HasParamSpec for std::cell::RefCell<T> {
    type Builder = T::Builder;
    fn builder() -> <Self as HasParamSpec>::Builder {
        T::builder()
    }
}
impl<T: ValueType + HasParamSpec> ParamStoreRead<T> for std::cell::RefCell<T> {
    fn get_value(&self) -> glib::Value {
        self.borrow().to_value()
    }
}
impl<T: ValueType + HasParamSpec + PartialEq> ParamStoreWrite<T> for std::cell::RefCell<T> {
    fn set(&self, value: T) -> bool {
        let old = self.replace(value);
        old != *self.borrow()
    }
}

impl<T: HasParamSpec> HasParamSpec for std::sync::Mutex<T> {
    type Builder = T::Builder;
    fn builder() -> <Self as HasParamSpec>::Builder {
        T::builder()
    }
}
impl<T: ValueType + HasParamSpec> ParamStoreRead<T> for std::sync::Mutex<T> {
    fn get_value(&self) -> glib::Value {
        self.lock().unwrap().to_value()
    }
}
impl<T: ValueType + HasParamSpec + PartialEq> ParamStoreWrite<T> for std::sync::Mutex<T> {
    fn set(&self, value: T) -> bool {
        let mut storage = self.lock().unwrap();
        let old = std::mem::replace(storage.deref_mut(), value);
        old != *storage
    }
}

impl<T: HasParamSpec> HasParamSpec for std::sync::RwLock<T> {
    type Builder = T::Builder;
    fn builder() -> <Self as HasParamSpec>::Builder {
        T::builder()
    }
}
impl<T: ValueType + HasParamSpec> ParamStoreRead<T> for std::sync::RwLock<T> {
    fn get_value(&self) -> glib::Value {
        self.read().unwrap().to_value()
    }
}
impl<T: ValueType + HasParamSpec + PartialEq> ParamStoreWrite<T> for std::sync::RwLock<T> {
    fn set(&self, value: T) -> bool {
        let mut storage = self.write().unwrap();
        let old = std::mem::replace(storage.deref_mut(), value);
        old != *storage
    }
}
