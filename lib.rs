use glib::{translate::*, value::ValueType, ParamFlags, ParamSpec, Value};
use std::ops::DerefMut;

pub use glib;
pub use gobject_impl_macros::*;

pub trait ParamSpecBuildable {
    type Builder;

    fn builder() -> <Self as ParamSpecBuildable>::Builder;
}

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
    type Type: ValueType;
}
pub trait ParamStoreRead: ParamStore {
    fn get_owned(&self) -> <Self as ParamStore>::Type;
}
pub trait ParamStoreReadValue: ParamStore {
    fn get_value(&self) -> glib::Value;
}
pub trait ParamStoreBorrow<'a>: ParamStore {
    type BorrowType;

    fn borrow(&'a self) -> Self::BorrowType;
}
pub trait ParamStoreWrite<'a>: ParamStore {
    fn set_owned(&'a self, value: <Self as ParamStore>::Type);
    fn set_value(&'a self, value: &'a Value) {
        self.set_owned(value.get().expect("invalid value for property"));
    }
}
pub trait ParamStoreWriteChanged<'a>: ParamStoreWrite<'a> {
    fn set_owned_checked(&'a self, value: <Self as ParamStore>::Type) -> bool;
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
impl<T> ParamStoreRead for std::cell::Cell<T>
where
    T: ValueType + Copy,
{
    fn get_owned(&self) -> <Self as ParamStore>::Type {
        std::cell::Cell::get(self)
    }
}
impl<T> ParamStoreReadValue for std::cell::Cell<T>
where
    T: ValueType + Copy,
{
    fn get_value(&self) -> glib::Value {
        self.get_owned().to_value()
    }
}
impl<'a, T> ParamStoreWrite<'a> for std::cell::Cell<T>
where
    T: ValueType,
{
    fn set_owned(&'a self, value: <Self as ParamStore>::Type) {
        self.replace(value);
    }
}
impl<'a, T> ParamStoreWriteChanged<'a> for std::cell::Cell<T>
where
    T: ValueType + PartialEq + Copy,
{
    fn set_owned_checked(&'a self, value: <Self as ParamStore>::Type) -> bool {
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
impl<T> ParamStoreRead for std::cell::RefCell<T>
where
    T: ValueType + Clone,
{
    fn get_owned(&self) -> <Self as ParamStore>::Type {
        self.borrow().clone()
    }
}
impl<T> ParamStoreReadValue for std::cell::RefCell<T>
where
    T: ValueType,
{
    fn get_value(&self) -> glib::Value {
        self.borrow().to_value()
    }
}
impl<'a, T> ParamStoreBorrow<'a> for std::cell::RefCell<T>
where
    T: ValueType + 'a,
{
    type BorrowType = std::cell::Ref<'a, T>;

    fn borrow(&'a self) -> Self::BorrowType {
        std::cell::RefCell::borrow(self)
    }
}
impl<'a, T> ParamStoreWrite<'a> for std::cell::RefCell<T>
where
    T: ValueType,
{
    fn set_owned(&'a self, value: <Self as ParamStore>::Type) {
        self.replace(value);
    }
}
impl<'a, T> ParamStoreWriteChanged<'a> for std::cell::RefCell<T>
where
    T: ValueType + PartialEq,
{
    fn set_owned_checked(&'a self, value: <Self as ParamStore>::Type) -> bool {
        let mut storage = self.borrow_mut();
        let old = std::mem::replace(storage.deref_mut(), value);
        old != *storage
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
impl<T> ParamStoreRead for std::sync::Mutex<T>
where
    T: ValueType + Clone,
{
    fn get_owned(&self) -> <Self as ParamStore>::Type {
        self.borrow().clone()
    }
}
impl<T> ParamStoreReadValue for std::sync::Mutex<T>
where
    T: ValueType,
{
    fn get_value(&self) -> glib::Value {
        self.borrow().to_value()
    }
}
impl<'a, T> ParamStoreBorrow<'a> for std::sync::Mutex<T>
where
    T: ValueType + 'a,
{
    type BorrowType = std::sync::MutexGuard<'a, T>;

    fn borrow(&'a self) -> Self::BorrowType {
        self.lock().unwrap()
    }
}
impl<'a, T> ParamStoreWrite<'a> for std::sync::Mutex<T>
where
    T: ValueType,
{
    fn set_owned(&'a self, value: <Self as ParamStore>::Type) {
        *self.lock().unwrap() = value;
    }
}
impl<'a, T> ParamStoreWriteChanged<'a> for std::sync::Mutex<T>
where
    T: ValueType + PartialEq,
{
    fn set_owned_checked(&'a self, value: <Self as ParamStore>::Type) -> bool {
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
impl<T> ParamStoreRead for std::sync::RwLock<T>
where
    T: ValueType + Clone,
{
    fn get_owned(&self) -> <Self as ParamStore>::Type {
        self.borrow().clone()
    }
}
impl<T> ParamStoreReadValue for std::sync::RwLock<T>
where
    T: ValueType,
{
    fn get_value(&self) -> glib::Value {
        self.borrow().to_value()
    }
}
impl<'a, T> ParamStoreBorrow<'a> for std::sync::RwLock<T>
where
    T: ValueType + 'a,
{
    type BorrowType = std::sync::RwLockReadGuard<'a, T>;

    fn borrow(&'a self) -> Self::BorrowType {
        self.read().unwrap()
    }
}
impl<'a, T> ParamStoreWrite<'a> for std::sync::RwLock<T>
where
    T: ValueType,
{
    fn set_owned(&'a self, value: <Self as ParamStore>::Type) {
        *self.write().unwrap() = value;
    }
}
impl<'a, T> ParamStoreWriteChanged<'a> for std::sync::RwLock<T>
where
    T: ValueType + PartialEq,
{
    fn set_owned_checked(&'a self, value: <Self as ParamStore>::Type) -> bool {
        let mut storage = self.write().unwrap();
        let old = std::mem::replace(storage.deref_mut(), value);
        old != *storage
    }
}

pub use glib::once_cell::unsync::OnceCell;

impl<T: ParamSpecBuildable> ParamSpecBuildable for OnceCell<T> {
    type Builder = T::Builder;

    fn builder() -> <Self as ParamSpecBuildable>::Builder {
        T::builder()
    }
}
impl<T: ValueType> ParamStore for OnceCell<T> {
    type Type = T;
}
impl<T> ParamStoreRead for OnceCell<T>
where
    T: ValueType + Clone,
{
    fn get_owned(&self) -> <Self as ParamStore>::Type {
        self.borrow().clone()
    }
}
impl<T> ParamStoreReadValue for OnceCell<T>
where
    T: ValueType,
{
    fn get_value(&self) -> glib::Value {
        self.borrow().to_value()
    }
}
impl<'a, T> ParamStoreBorrow<'a> for OnceCell<T>
where
    T: ValueType + 'a,
{
    type BorrowType = &'a T;

    fn borrow(&'a self) -> Self::BorrowType {
        self.get()
            .unwrap_or_else(|| panic!("`get()` called on uninitialized OnceCell"))
    }
}
impl<'a, T> ParamStoreWrite<'a> for OnceCell<T>
where
    T: ValueType,
{
    fn set_owned(&'a self, value: <Self as ParamStore>::Type) {
        self.set(value)
            .unwrap_or_else(|_| panic!("set() called on initialized OnceCell"));
    }
}
impl<'a, T> ParamStoreWriteChanged<'a> for OnceCell<T>
where
    T: ValueType + PartialEq + Copy,
{
    fn set_owned_checked(&'a self, value: <Self as ParamStore>::Type) -> bool {
        self.set_owned(value);
        true
    }
}

pub use glib::once_cell::sync::OnceCell as SyncOnceCell;

impl<T: ParamSpecBuildable> ParamSpecBuildable for SyncOnceCell<T> {
    type Builder = T::Builder;

    fn builder() -> <Self as ParamSpecBuildable>::Builder {
        T::builder()
    }
}
impl<T: ValueType> ParamStore for SyncOnceCell<T> {
    type Type = T;
}
impl<T> ParamStoreRead for SyncOnceCell<T>
where
    T: ValueType + Clone,
{
    fn get_owned(&self) -> <Self as ParamStore>::Type {
        self.borrow().clone()
    }
}
impl<T> ParamStoreReadValue for SyncOnceCell<T>
where
    T: ValueType,
{
    fn get_value(&self) -> glib::Value {
        self.borrow().to_value()
    }
}
impl<'a, T> ParamStoreBorrow<'a> for SyncOnceCell<T>
where
    T: ValueType + 'a,
{
    type BorrowType = &'a T;

    fn borrow(&'a self) -> Self::BorrowType {
        self.get()
            .unwrap_or_else(|| panic!("`get()` called on uninitialized OnceCell"))
    }
}
impl<'a, T> ParamStoreWrite<'a> for SyncOnceCell<T>
where
    T: ValueType,
{
    fn set_owned(&'a self, value: <Self as ParamStore>::Type) {
        self.set(value)
            .unwrap_or_else(|_| panic!("set() called on initialized OnceCell"));
    }
}
impl<'a, T> ParamStoreWriteChanged<'a> for SyncOnceCell<T>
where
    T: ValueType + PartialEq + Copy,
{
    fn set_owned_checked(&'a self, value: <Self as ParamStore>::Type) -> bool {
        self.set_owned(value);
        true
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct ConstructCell<T>(std::cell::RefCell<Option<T>>);

impl<T> ConstructCell<T> {
    pub fn new(value: T) -> Self {
        Self(std::cell::RefCell::new(Some(value)))
    }
    pub fn new_empty() -> Self {
        Self(std::cell::RefCell::new(None))
    }
    pub fn borrow(&self) -> std::cell::Ref<'_, T> {
        std::cell::Ref::map(self.0.borrow(), |r| r.as_ref().unwrap())
    }
    pub fn borrow_mut(&self) -> std::cell::RefMut<'_, T> {
        std::cell::RefMut::map(self.0.borrow_mut(), |r| r.as_mut().unwrap())
    }
    pub fn replace(&self, t: T) -> Option<T> {
        self.0.replace(Some(t))
    }
}

impl<T> Default for ConstructCell<T> {
    fn default() -> Self {
        Self::new_empty()
    }
}

impl<T: ParamSpecBuildable> ParamSpecBuildable for ConstructCell<T> {
    type Builder = T::Builder;

    fn builder() -> <Self as ParamSpecBuildable>::Builder {
        T::builder()
    }
}
impl<T: ValueType> ParamStore for ConstructCell<T> {
    type Type = T;
}
impl<T> ParamStoreRead for ConstructCell<T>
where
    T: ValueType + Clone,
{
    fn get_owned(&self) -> <Self as ParamStore>::Type {
        self.borrow().clone()
    }
}
impl<T> ParamStoreReadValue for ConstructCell<T>
where
    T: ValueType,
{
    fn get_value(&self) -> glib::Value {
        self.borrow().to_value()
    }
}
impl<'a, T> ParamStoreBorrow<'a> for ConstructCell<T>
where
    T: ValueType + 'a,
{
    type BorrowType = std::cell::Ref<'a, T>;

    fn borrow(&'a self) -> Self::BorrowType {
        ConstructCell::borrow(self)
    }
}
impl<'a, T> ParamStoreWrite<'a> for ConstructCell<T>
where
    T: ValueType,
{
    fn set_owned(&'a self, value: <Self as ParamStore>::Type) {
        self.replace(value);
    }
}
impl<'a, T> ParamStoreWriteChanged<'a> for ConstructCell<T>
where
    T: ValueType + PartialEq,
{
    fn set_owned_checked(&'a self, value: <Self as ParamStore>::Type) -> bool {
        let mut storage = self.borrow_mut();
        let old = std::mem::replace(storage.deref_mut(), value);
        old != *storage
    }
}
