use std::ops::{Deref, DerefMut};

use glib::{translate::*, value::ValueType, ParamFlags, ParamSpec, ToValue, Value};

pub use glib;
pub use gobject_impl_macros::*;

pub trait ParamReadType<'a> {
    type ReadType;
    type ReadFastType;
}

pub trait ParamWriteType<'a> {
    type WriteType;
    type WriteFastType;
}

pub trait ParamSpecBuildable {
    type Builder;
    fn builder() -> <Self as ParamSpecBuildable>::Builder;
}

macro_rules! define_defaulted {
    ($ty:ty, $name:ty, $builder_name:ident) => {
        impl<'a> ParamReadType<'a> for $ty {
            type ReadType = Self;
            type ReadFastType = Self;
        }
        impl<'a> ParamWriteType<'a> for $ty {
            type WriteType = Self;
            type WriteFastType = Self;
        }
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
        impl<'a> ParamReadType<'a> for $ty {
            type ReadType = Self;
            type ReadFastType = Self;
        }
        impl<'a> ParamWriteType<'a> for $ty {
            type WriteType = Self;
            type WriteFastType = Self;
        }
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

define_defaulted!(bool, glib::ParamSpecBoolean, ParamSpecBooleanBuilder);
define_numeric!(i8, glib::ParamSpecChar, ParamSpecCharBuilder);
define_numeric!(u8, glib::ParamSpecUChar, ParamSpecUCharBuilder);
define_numeric!(i32, glib::ParamSpecInt, ParamSpecIntBuilder);
define_numeric!(u32, glib::ParamSpecUInt, ParamSpecUIntBuilder);
define_numeric!(i64, glib::ParamSpecInt64, ParamSpecInt64Builder);
define_numeric!(u64, glib::ParamSpecUInt64, ParamSpecUInt64Builder);
define_numeric!(f32, glib::ParamSpecFloat, ParamSpecFloatBuilder);
define_numeric!(f64, glib::ParamSpecDouble, ParamSpecDoubleBuilder);
// TODO
//define_defaulted!(char, glib::ParamSpecUnichar, ParamSpecUnicharBuilder);

impl<T: ParamSpecBuildable> ParamSpecBuildable for Option<T> {
    type Builder = T::Builder;
    fn builder() -> T::Builder {
        T::builder()
    }
}

impl<'a> ParamReadType<'a> for String {
    type ReadType = Self;
    type ReadFastType = &'a str;
}
impl<'a> ParamWriteType<'a> for String {
    type WriteType = Self;
    type WriteFastType = &'a str;
}
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

pub struct ParamSpecParamBuilder {
    subtype: glib::Type,
}
impl ParamSpecParamBuilder {
    pub fn subtype<T: glib::ParamSpecType>(mut self) -> Self {
        self.subtype = T::static_type();
        self
    }
    pub fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        glib::ParamSpecParam::new(name, nick, blurb, self.subtype, flags)
    }
}
impl ParamSpecBuildable for ParamSpec {
    type Builder = ParamSpecParamBuilder;
    fn builder() -> ParamSpecParamBuilder {
        ParamSpecParamBuilder {
            subtype: glib::Type::PARAM_SPEC,
        }
    }
}

pub struct ParamSpecPointerBuilder {}
impl ParamSpecPointerBuilder {
    pub fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        glib::ParamSpecPointer::new(name, nick, blurb, flags)
    }
}
/*
 * TODO
impl<T> ParamSpecBuildable for glib::types::Pointer {
    type Builder = ParamSpecPointerBuilder;
    fn builder() -> ParamSpecPointerBuilder {
        ParamSpecPointerBuilder {}
    }
}
*/

pub struct ParamSpecGTypeBuilder {
    subtype: glib::Type,
}
impl ParamSpecGTypeBuilder {
    pub fn subtype<T: glib::StaticType>(mut self) -> Self {
        self.subtype = T::static_type();
        self
    }
    pub fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        glib::ParamSpecGType::new(name, nick, blurb, self.subtype, flags)
    }
}
impl ParamSpecBuildable for glib::Type {
    type Builder = ParamSpecGTypeBuilder;
    fn builder() -> ParamSpecGTypeBuilder {
        ParamSpecGTypeBuilder {
            subtype: glib::Type::UNIT,
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
            type_: glib::Type::ENUM,
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
            type_: glib::Type::FLAGS,
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
            type_: glib::Type::BOXED,
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
            type_: glib::Type::OBJECT,
        }
    }
}
impl ParamSpecObjectBuilder {
    pub fn type_<T: glib::StaticType>(mut self) -> Self {
        self.type_ = T::static_type();
        self
    }
    pub fn build(self, name: &str, nick: &str, blurb: &str, flags: ParamFlags) -> ParamSpec {
        glib::ParamSpecObject::new(name, nick, blurb, self.type_, flags)
    }
}

pub struct ParamSpecVariantBuilder {
    variant: &'static glib::VariantTy,
    default: Option<&'static str>,
}
impl ParamSpecVariantBuilder {
    pub fn variant(mut self, variant: &'static str) -> Self {
        self.variant = glib::VariantTy::new(variant).unwrap();
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
                    self.variant.to_glib_none().0,
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
        glib::ParamSpecVariant::new(name, nick, blurb, self.variant, default.as_ref(), flags)
    }
}
impl ParamSpecBuildable for glib::Variant {
    type Builder = ParamSpecVariantBuilder;
    fn builder() -> ParamSpecVariantBuilder {
        ParamSpecVariantBuilder {
            variant: glib::VariantTy::ANY,
            default: None,
        }
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
pub trait ParamStoreReadRef<'a>: ParamStore {
    type Ref: Deref<Target = <Self as ParamStore>::Type>;
    fn get_ref(&'a self) -> Self::Ref;
}
pub trait ParamStoreWrite: ParamStore {
    fn set(&self, value: Self::Type);
    fn set_ref<V: ToOwned<Owned = Self::Type>>(&self, value: &V) {
        self.set(value.to_owned())
    }
    fn set_value(&self, value: &Value) {
        let v = value.get_owned().expect("Invalid value for property");
        self.set(v);
    }
}
pub trait ParamStoreWriteChanged: ParamStore {
    fn set_checked(&self, value: Self::Type) -> bool;
    fn set_ref_checked<V: ToOwned<Owned = Self::Type>>(&self, value: &V) -> bool {
        self.set_checked(value.to_owned())
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
impl<T: ValueType + Copy> ParamStoreRead for std::cell::Cell<T> {
    fn get(&self) -> T {
        std::cell::Cell::get(self)
    }
}
impl<T: ValueType> ParamStoreWrite for std::cell::Cell<T> {
    fn set(&self, value: T) {
        self.replace(value);
    }
}
impl<T: ValueType + PartialEq + Copy> ParamStoreWriteChanged for std::cell::Cell<T> {
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
impl<T: ValueType + Clone> ParamStoreRead for std::cell::RefCell<T> {
    fn get(&self) -> T {
        self.borrow().clone()
    }
}
impl<'a, T: ValueType> ParamStoreReadRef<'a> for std::cell::RefCell<T> {
    type Ref = std::cell::Ref<'a, T>;
    fn get_ref(&'a self) -> Self::Ref {
        self.borrow()
    }
}
impl<T: ValueType> ParamStoreWrite for std::cell::RefCell<T> {
    fn set(&self, value: T) {
        self.replace(value);
    }
}
impl<T: ValueType + PartialEq> ParamStoreWriteChanged for std::cell::RefCell<T> {
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
impl<T: ValueType + Clone> ParamStoreRead for std::sync::Mutex<T> {
    fn get(&self) -> T {
        self.lock().unwrap().clone()
    }
}
impl<'a, T: ValueType> ParamStoreReadRef<'a> for std::sync::Mutex<T> {
    type Ref = std::sync::MutexGuard<'a, T>;
    fn get_ref(&'a self) -> Self::Ref {
        self.lock().unwrap()
    }
}
impl<T: ValueType> ParamStoreWrite for std::sync::Mutex<T> {
    fn set(&self, value: T) {
        *self.lock().unwrap() = value;
    }
}
impl<T: ValueType + PartialEq> ParamStoreWriteChanged for std::sync::Mutex<T> {
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
impl<T: ValueType + Clone> ParamStoreRead for std::sync::RwLock<T> {
    fn get(&self) -> T {
        self.read().unwrap().clone()
    }
}
impl<'a, T: ValueType> ParamStoreReadRef<'a> for std::sync::RwLock<T> {
    type Ref = std::sync::RwLockReadGuard<'a, T>;
    fn get_ref(&'a self) -> Self::Ref {
        self.read().unwrap()
    }
}
impl<T: ValueType> ParamStoreWrite for std::sync::RwLock<T> {
    fn set(&self, value: T) {
        *self.write().unwrap() = value;
    }
}
impl<T: ValueType + PartialEq> ParamStoreWriteChanged for std::sync::RwLock<T> {
    fn set_checked(&self, value: T) -> bool {
        let mut storage = self.write().unwrap();
        let old = std::mem::replace(storage.deref_mut(), value);
        old != *storage
    }
}

impl<T: ParamSpecBuildable> ParamSpecBuildable for glib::once_cell::unsync::OnceCell<T> {
    type Builder = T::Builder;
    fn builder() -> <Self as ParamSpecBuildable>::Builder {
        T::builder()
    }
}
impl<T: ValueType> ParamStore for glib::once_cell::unsync::OnceCell<T> {
    type Type = T;
}
impl<T: ValueType + Clone> ParamStoreRead for glib::once_cell::unsync::OnceCell<T> {
    fn get(&self) -> T {
        self.get()
            .unwrap_or_else(|| panic!("`get()` called on uninitialized OnceCell"))
            .clone()
    }
}
impl<'a, T: ValueType> ParamStoreReadRef<'a> for glib::once_cell::unsync::OnceCell<T> {
    type Ref = &'a T;
    fn get_ref(&'a self) -> Self::Ref {
        self.get()
            .unwrap_or_else(|| panic!("`get()` called on uninitialized OnceCell"))
    }
}
impl<T: ValueType> ParamStoreWrite for glib::once_cell::unsync::OnceCell<T> {
    fn set(&self, value: T) {
        self.set(value)
            .unwrap_or_else(|_| panic!("set() called on initialized OnceCell"));
    }
}
impl<T: ValueType> ParamStoreWriteChanged for glib::once_cell::unsync::OnceCell<T> {
    fn set_checked(&self, value: T) -> bool {
        self.set(value)
            .unwrap_or_else(|_| panic!("set() called on initialized OnceCell"));
        true
    }
}

impl<T: ParamSpecBuildable> ParamSpecBuildable for glib::once_cell::sync::OnceCell<T> {
    type Builder = T::Builder;
    fn builder() -> <Self as ParamSpecBuildable>::Builder {
        T::builder()
    }
}
impl<T: ValueType> ParamStore for glib::once_cell::sync::OnceCell<T> {
    type Type = T;
}
impl<T: ValueType + Clone> ParamStoreRead for glib::once_cell::sync::OnceCell<T> {
    fn get(&self) -> T {
        self.get()
            .unwrap_or_else(|| panic!("`get()` called on uninitialized OnceCell"))
            .clone()
    }
}
impl<'a, T: ValueType> ParamStoreReadRef<'a> for glib::once_cell::sync::OnceCell<T> {
    type Ref = &'a T;
    fn get_ref(&'a self) -> Self::Ref {
        self.get()
            .unwrap_or_else(|| panic!("`get()` called on uninitialized OnceCell"))
    }
}
impl<T: ValueType> ParamStoreWrite for glib::once_cell::sync::OnceCell<T> {
    fn set(&self, value: T) {
        self.set(value)
            .unwrap_or_else(|_| panic!("set() called on initialized OnceCell"));
    }
}
impl<T: ValueType> ParamStoreWriteChanged for glib::once_cell::sync::OnceCell<T> {
    fn set_checked(&self, value: T) -> bool {
        self.set(value)
            .unwrap_or_else(|_| panic!("set() called on initialized OnceCell"));
        true
    }
}
