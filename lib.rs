use std::ops::DerefMut;

use glib::{translate::*, value::ValueType, ParamFlags, ParamSpec, ToValue, Value};

pub use glib;
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
            pub fn build(
                self,
                name: &str,
                nick: &str,
                blurb: &str,
                flags: ParamFlags,
            ) -> ParamSpec {
                <$name>::new(name, nick, blurb, self.default, flags)
            }
        }
        impl ParamSpecBuildable for $ty {
            type Builder = $builder_name;
            fn builder() -> $builder_name {
                $builder_name {
                    default: Default::default(),
                }
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
            pub fn build(
                self,
                name: &str,
                nick: &str,
                blurb: &str,
                flags: ParamFlags,
            ) -> ParamSpec {
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
        impl ParamSpecBuildable for $ty {
            type Builder = $builder_name;
            fn builder() -> $builder_name {
                $builder_name {
                    minimum: <$ty>::MIN,
                    maximum: <$ty>::MAX,
                    default: Default::default(),
                }
            }
        }
    };
}

pub trait ParamSpecBuildable {
    type Builder;
    fn builder() -> <Self as ParamSpecBuildable>::Builder;
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
    pub fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        glib::ParamSpecString::new(name, nick, blurb, self.default, flags)
    }
}
impl ParamSpecBuildable for String {
    type Builder = ParamSpecStringBuilder;
    fn builder() -> ParamSpecStringBuilder {
        ParamSpecStringBuilder { default: None }
    }
}
impl ParamSpecBuildable for Option<String> {
    type Builder = ParamSpecStringBuilder;
    fn builder() -> ParamSpecStringBuilder {
        <String as ParamSpecBuildable>::builder()
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
    pub fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        if self.type_ == glib::Type::UNIT {
            panic!(
                "property `{}` must specify a type implementing glib::ParamSpecType",
                name
            );
        }
        glib::ParamSpecParam::new(name, nick, blurb, self.type_, flags)
    }
}
impl ParamSpecBuildable for ParamSpec {
    type Builder = ParamSpecParamBuilder;
    fn builder() -> ParamSpecParamBuilder {
        ParamSpecParamBuilder {
            type_: glib::Type::UNIT,
        }
    }
}
impl ParamSpecBuildable for Option<ParamSpec> {
    type Builder = ParamSpecParamBuilder;
    fn builder() -> ParamSpecParamBuilder {
        <ParamSpec as ParamSpecBuildable>::builder()
    }
}

pub struct ParamSpecPointerBuilder {}
impl ParamSpecPointerBuilder {
    pub fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        glib::ParamSpecPointer::new(name, nick, blurb, flags)
    }
}
impl<T> ParamSpecBuildable for *mut T {
    type Builder = ParamSpecPointerBuilder;
    fn builder() -> ParamSpecPointerBuilder {
        ParamSpecPointerBuilder {}
    }
}
impl<T> ParamSpecBuildable for *const T {
    type Builder = ParamSpecPointerBuilder;
    fn builder() -> ParamSpecPointerBuilder {
        ParamSpecPointerBuilder {}
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
    pub fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        glib::ParamSpecGType::new(name, nick, blurb, self.type_, flags)
    }
}
impl ParamSpecBuildable for glib::Type {
    type Builder = ParamSpecGTypeBuilder;
    fn builder() -> ParamSpecGTypeBuilder {
        ParamSpecGTypeBuilder {
            type_: glib::Type::UNIT,
        }
    }
}

pub struct ParamSpecEnumBuilder {
    type_: glib::Type,
    default: i32,
}
impl Default for ParamSpecEnumBuilder {
    fn default() -> Self {
        Self {
            type_: glib::Type::UNIT,
            default: 0,
        }
    }
}
impl ParamSpecEnumBuilder {
    pub fn type_<T: glib::StaticType>(mut self) -> Self {
        self.type_ = T::static_type();
        self
    }
    pub fn default(mut self, value: i32) -> Self {
        self.default = value;
        self
    }
    pub fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        glib::ParamSpecEnum::new(name, nick, blurb, self.type_, self.default, flags)
    }
}

pub struct ParamSpecFlagsBuilder {
    type_: glib::Type,
    default: u32,
}
impl Default for ParamSpecFlagsBuilder {
    fn default() -> Self {
        Self {
            type_: glib::Type::UNIT,
            default: 0,
        }
    }
}
impl ParamSpecFlagsBuilder {
    pub fn type_<T: glib::StaticType>(mut self) -> Self {
        self.type_ = T::static_type();
        self
    }
    pub fn default(mut self, value: u32) -> Self {
        self.default = value;
        self
    }
    pub fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        glib::ParamSpecFlags::new(name, nick, blurb, self.type_, self.default, flags)
    }
}

pub struct ParamSpecBoxedBuilder {
    type_: glib::Type,
}
impl Default for ParamSpecBoxedBuilder {
    fn default() -> Self {
        Self {
            type_: glib::Type::UNIT,
        }
    }
}
impl ParamSpecBoxedBuilder {
    pub fn type_<T: glib::StaticType>(mut self) -> Self {
        self.type_ = T::static_type();
        self
    }
    pub fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        glib::ParamSpecBoxed::new(name, nick, blurb, self.type_, flags)
    }
}

pub struct ParamSpecObjectBuilder {
    type_: glib::Type,
}
impl Default for ParamSpecObjectBuilder {
    fn default() -> Self {
        Self {
            type_: glib::Type::UNIT,
        }
    }
}
impl ParamSpecObjectBuilder {
    pub fn new() -> Self {
        Self {
            type_: glib::Type::UNIT,
        }
    }
    pub fn type_<T: glib::StaticType>(mut self) -> Self {
        self.type_ = T::static_type();
        self
    }
    pub fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        glib::ParamSpecObject::new(name, nick, blurb, self.type_, flags)
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
    pub fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
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
impl ParamSpecBuildable for glib::Variant {
    type Builder = ParamSpecVariantBuilder;
    fn builder() -> ParamSpecVariantBuilder {
        ParamSpecVariantBuilder {
            type_: glib::VariantTy::ANY,
            default: None,
        }
    }
}
impl ParamSpecBuildable for Option<glib::Variant> {
    type Builder = ParamSpecVariantBuilder;
    fn builder() -> ParamSpecVariantBuilder {
        <glib::Variant as ParamSpecBuildable>::builder()
    }
}

pub trait ParamStore {
    type Type: ValueType + ToValue;
}
pub trait ParamStoreRead: ParamStore {
    fn get(&self) -> Self::Type;
    fn get_value(&self) -> glib::Value {
        self.get().to_value()
    }
}
pub trait ParamStoreWrite: ParamStore {
    fn set(&self, value: Self::Type);
    fn set_checked(&self, value: Self::Type) -> bool;
    fn set_value(&self, value: &Value) {
        let v = value.get_owned().expect("Invalid value for property");
        self.set(v);
    }
    fn set_value_checked(&self, value: &Value) -> bool {
        let v = value.get_owned().expect("Invalid value for property");
        self.set_checked(v)
    }
}

impl<T: ParamSpecBuildable> ParamSpecBuildable for std::cell::Cell<T> {
    type Builder = T::Builder;
    fn builder() -> <Self as ParamSpecBuildable>::Builder {
        T::builder()
    }
}
impl<T: ValueType> ParamStore for std::cell::Cell<T> {
    type Type = T;
}
impl<T: ValueType + Copy + ParamSpecBuildable> ParamStoreRead for std::cell::Cell<T> {
    fn get(&self) -> T {
        std::cell::Cell::get(self)
    }
}
impl<T: ValueType + ParamSpecBuildable + PartialEq + Copy> ParamStoreWrite for std::cell::Cell<T> {
    fn set(&self, value: T) {
        self.replace(value);
    }
    fn set_checked(&self, value: T) -> bool {
        let old = self.replace(value);
        old != self.get()
    }
}

impl<T: ParamSpecBuildable> ParamSpecBuildable for std::cell::RefCell<T> {
    type Builder = T::Builder;
    fn builder() -> <Self as ParamSpecBuildable>::Builder {
        T::builder()
    }
}
impl<T: ValueType> ParamStore for std::cell::RefCell<T> {
    type Type = T;
}
impl<T: ValueType + ParamSpecBuildable + Clone> ParamStoreRead for std::cell::RefCell<T> {
    fn get(&self) -> T {
        self.borrow().clone()
    }
}
impl<T: ValueType + ParamSpecBuildable + PartialEq> ParamStoreWrite for std::cell::RefCell<T> {
    fn set(&self, value: T) {
        self.replace(value);
    }
    fn set_checked(&self, value: T) -> bool {
        let old = self.replace(value);
        old != *self.borrow()
    }
}

impl<T: ParamSpecBuildable> ParamSpecBuildable for std::sync::Mutex<T> {
    type Builder = T::Builder;
    fn builder() -> <Self as ParamSpecBuildable>::Builder {
        T::builder()
    }
}
impl<T: ValueType> ParamStore for std::sync::Mutex<T> {
    type Type = T;
}
impl<T: ValueType + ParamSpecBuildable + Clone> ParamStoreRead for std::sync::Mutex<T> {
    fn get(&self) -> T {
        self.lock().unwrap().clone()
    }
}
impl<T: ValueType + ParamSpecBuildable + PartialEq> ParamStoreWrite for std::sync::Mutex<T> {
    fn set(&self, value: T) {
        *self.lock().unwrap() = value;
    }
    fn set_checked(&self, value: T) -> bool {
        let mut storage = self.lock().unwrap();
        let old = std::mem::replace(storage.deref_mut(), value);
        old != *storage
    }
}

impl<T: ParamSpecBuildable> ParamSpecBuildable for std::sync::RwLock<T> {
    type Builder = T::Builder;
    fn builder() -> <Self as ParamSpecBuildable>::Builder {
        T::builder()
    }
}
impl<T: ValueType> ParamStore for std::sync::RwLock<T> {
    type Type = T;
}
impl<T: ValueType + ParamSpecBuildable + Clone> ParamStoreRead for std::sync::RwLock<T> {
    fn get(&self) -> T {
        self.read().unwrap().clone()
    }
}
impl<T: ValueType + ParamSpecBuildable + PartialEq> ParamStoreWrite for std::sync::RwLock<T> {
    fn set(&self, value: T) {
        *self.write().unwrap() = value;
    }
    fn set_checked(&self, value: T) -> bool {
        let mut storage = self.write().unwrap();
        let old = std::mem::replace(storage.deref_mut(), value);
        old != *storage
    }
}
