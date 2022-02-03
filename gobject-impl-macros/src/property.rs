use heck::{ToKebabCase, ToSnakeCase};
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, ToTokens};
use syn::Token;

mod keywords {
    // property keywords
    syn::custom_keyword!(property);

    // prop attributes
    syn::custom_keyword!(skip);
    syn::custom_keyword!(get);
    syn::custom_keyword!(set);
    syn::custom_keyword!(name);
    syn::custom_keyword!(nick);
    syn::custom_keyword!(blurb);
    syn::custom_keyword!(minimum);
    syn::custom_keyword!(maximum);
    syn::custom_keyword!(default);
    syn::custom_keyword!(flags);
    syn::custom_keyword!(boxed);
    syn::custom_keyword!(object);
    syn::custom_keyword!(variant);
    syn::custom_keyword!(delegate);
    syn::custom_keyword!(nullable);
    syn::custom_keyword!(construct);
    syn::custom_keyword!(construct_only);
    syn::custom_keyword!(lax_validation);
    syn::custom_keyword!(explicit_notify);
    syn::custom_keyword!(deprecated);
}

bitflags::bitflags! {
    pub struct PropertyFlags: u32 {
        const READABLE        = 1 << 0;
        const WRITABLE        = 1 << 1;
        const CONSTRUCT       = 1 << 2;
        const CONSTRUCT_ONLY  = 1 << 3;
        const LAX_VALIDATION  = 1 << 4;
        const EXPLICIT_NOTIFY = 1 << 30;
        const DEPRECATED      = 1 << 31;
    }
}

impl PropertyFlags {
    fn tokens(&self, glib: &TokenStream2, readable: bool, writable: bool) -> TokenStream2 {
        let count = Self::empty().bits().leading_zeros() - Self::all().bits().leading_zeros();
        let mut flags = vec![];
        if readable {
            flags.push(quote! { #glib::ParamFlags::READABLE });
        }
        if writable {
            flags.push(quote! { #glib::ParamFlags::WRITABLE });
        }
        for i in 0..count {
            let flag = Self::from_bits(1 << i).unwrap();
            if self.contains(flag) {
                let flag = format!("{:?}", flag);
                let flag = format_ident!("{}", flag);
                flags.push(quote! { #glib::ParamFlags::#flag });
            }
        }
        quote! { #(#flags)|* }
    }
}

pub enum PropertyType {
    Unspecified(syn::Type),
    Enum(syn::Type),
    Flags(syn::Type),
    Boxed(syn::Type),
    Object(syn::Type),
    Variant(Option<String>),
}

impl Default for PropertyType {
    fn default() -> Self {
        Self::Unspecified(syn::Type::Verbatim(Default::default()))
    }
}

impl PropertyType {
    pub fn inner_type(&self) -> Option<&syn::Type> {
        let ty = match self {
            PropertyType::Unspecified(ty) => ty,
            PropertyType::Enum(ty) => ty,
            PropertyType::Flags(ty) => ty,
            PropertyType::Boxed(ty) => ty,
            PropertyType::Object(ty) => ty,
            PropertyType::Variant(_) => return None,
        };
        let path = match ty {
            syn::Type::Path(syn::TypePath { path, .. }) => path,
            _ => return None,
        };
        let seg = path.segments.last()?;
        let bracketed = match &seg.arguments {
            syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                args,
                ..
            }) => args,
            _ => return None,
        };
        let arg = bracketed.last()?;
        let ty = match arg {
            syn::GenericArgument::Type(ty) => ty,
            _ => return None,
        };
        Some(ty)
    }
}

pub enum PropertyVirtual {
    Virtual(Token![virtual]),
    Delegate(Box<syn::Expr>),
}

pub struct Property {
    pub field: Option<syn::Field>,
    pub public: bool,
    pub skip: bool,
    pub type_: PropertyType,
    pub virtual_: Option<PropertyVirtual>,
    pub nullable: Option<keywords::nullable>,
    pub get: Option<Option<syn::Path>>,
    pub set: Option<Option<syn::Path>>,
    pub name: Option<String>,
    pub nick: Option<String>,
    pub blurb: Option<String>,
    pub minimum: Option<syn::Lit>,
    pub maximum: Option<syn::Lit>,
    pub default: Option<syn::Lit>,
    pub flags: PropertyFlags,
}

impl Property {
    pub fn create(&self, go: &syn::Ident) -> TokenStream2 {
        let glib = quote! { #go::glib };
        let name = self.name.as_ref().unwrap();
        let nick = self.nick.as_ref().unwrap();
        let blurb = self.blurb.as_ref().unwrap();
        let flags = self
            .flags
            .tokens(&glib, self.get.is_some(), self.set.is_some());
        match &self.type_ {
            PropertyType::Unspecified(ty) => {
                let minimum = self.minimum.as_ref().map(|d| quote! { .minimum(#d) });
                let maximum = self.maximum.as_ref().map(|d| quote! { .maximum(#d) });
                let default = self.default.as_ref().map(|d| quote! { .default(#d) });
                quote! {
                    <#ty as #go::HasParamSpec>::builder()
                    #minimum
                    #maximum
                    #default
                    .build(#name, #nick, #blurb, #flags)
                }
            }
            PropertyType::Enum(ty) => {
                let default = self
                    .default
                    .as_ref()
                    .map(|p| p.to_token_stream())
                    .unwrap_or_else(|| quote! { 0 });
                quote! {
                    #glib::ParamSpecEnum::new(
                        #name, #nick, #blurb,
                        <#ty as #glib::StaticType>::static_type(),
                        #default,
                        #flags
                    )
                }
            }
            PropertyType::Flags(ty) => {
                let default = self
                    .default
                    .as_ref()
                    .map(|p| p.to_token_stream())
                    .unwrap_or_else(|| quote! { 0 });
                quote! {
                    #glib::ParamSpecFlags::new(
                        #name, #nick, #blurb,
                        <#ty as #glib::StaticType>::static_type(),
                        #default,
                        #flags
                    )
                }
            }
            PropertyType::Boxed(ty) => quote! {
                #glib::ParamSpecBoxed::new(
                    #name, #nick, #blurb,
                    <#ty as #glib::StaticType>::static_type(),
                    #flags
                )
            },
            PropertyType::Object(ty) => quote! {
                #glib::ParamSpecObject::new(
                    #name, #nick, #blurb,
                    <#ty as #glib::StaticType>::static_type(),
                    #flags
                )
            },
            PropertyType::Variant(element) => {
                let element = element.as_ref().map(|e| quote! { .type_(#e) });
                let default = self.default.as_ref().map(|d| quote! { .default(#d) });
                quote! {
                    <#glib::Variant as #go::HasParamSpec>::builder()
                    #element
                    #default
                    .build(#name, #nick, #blurb, #flags)
                }
            }
        }
    }
    fn name_ident(&self) -> String {
        self.name.as_ref().unwrap().to_snake_case()
    }
    pub fn get_impl(
        &self,
        index: usize,
        trait_name: &syn::Ident,
        glib: &TokenStream2,
    ) -> Option<TokenStream2> {
        self.get.as_ref().map(|expr| {
            let expr = expr
                .as_ref()
                .map(|expr| quote! { #expr() })
                .unwrap_or_else(|| {
                    let method_name = format_ident!("{}", self.name_ident());
                    quote! { <<Self as #glib::subclass::types::ObjectSubclass>::Type as #trait_name>::#method_name(obj) }
                });
            quote! { #index => #glib::ToValue::to_value(#expr) }
        })
    }
    pub fn getter_prototype(&self) -> Option<TokenStream2> {
        matches!(self.get, Some(None)).then(|| {
            let method_name = format_ident!("{}", self.name_ident());
            let ty = self.type_.inner_type().unwrap();
            quote! { fn #method_name(&self) -> #ty }
        })
    }
    pub fn getter_definition(&self, go: &syn::Ident) -> Option<TokenStream2> {
        matches!(self.get, Some(None)).then(|| {
            let proto = self.getter_prototype().unwrap();
            let field = self.field_storage();
            let ty = self.type_.inner_type().unwrap();
            quote! {
                #proto {
                    #go::ParamStoreWrite::get_value(&#field).get::<#ty>().unwrap()
                }
            }
        })
    }
    fn field_storage(&self) -> TokenStream2 {
        if let Some(PropertyVirtual::Delegate(delegate)) = &self.virtual_ {
            quote! { #delegate }
        } else {
            let field = self.field.as_ref().unwrap().ident.as_ref().unwrap();
            quote! { self.#field }
        }
    }
    pub fn set_impl(
        &self,
        index: usize,
        trait_name: &syn::Ident,
        go: &syn::Ident,
    ) -> Option<TokenStream2> {
        self.get.as_ref().map(|expr| {
            let ty = self.type_.inner_type().unwrap();
            let set = if let Some(expr) = &expr {
                quote! { #expr(value.get::<#ty>().unwrap()); }
            } else if self.flags.contains(PropertyFlags::EXPLICIT_NOTIFY) {
                let method_name = format_ident!("set_{}", self.name_ident());
                quote! {
                    <<Self as #go::glib::subclass::types::ObjectSubclass>::Type as #trait_name>::#method_name(obj, value.get::<#ty>().unwrap());
                }
            } else {
                let field = self.field_storage();
                quote! { #go::ParamStoreWrite::set_value(&#field, value); }
            };
            quote! { #index => { #set } }
        })
    }
    pub fn setter_prototype(&self) -> Option<TokenStream2> {
        matches!(self.set, Some(None)).then(|| {
            let method_name = format_ident!("set_{}", self.name_ident());
            let ty = self.type_.inner_type().unwrap();
            quote! { fn #method_name(&self, value: #ty) }
        })
    }
    pub fn setter_definition(
        &self,
        index: usize,
        trait_name: &TokenStream2,
        go: &syn::Ident,
    ) -> Option<TokenStream2> {
        matches!(self.set, Some(None)).then(|| {
            let proto = self.setter_prototype().unwrap();
            let body = if self.flags.contains(PropertyFlags::EXPLICIT_NOTIFY) {
                let field = self.field_storage();
                quote! {
                    if #go::ParamStoreWrite::set(&#field, &value) {
                        self.notify_by_pspec(&<<Self as #go::glib::object::ObjectSubclassIs>::Subclass as #trait_name>::properties()[#index]);
                    }
                }
            } else {
                let name = self.name.as_ref().unwrap();
                quote! {
                    self.set_property(#name, #go::glib::ToValue::to_value(&value));
                }
            };
            quote! {
                #proto {
                    #body
                }
            }
        })
    }
    pub fn pspec_prototype(&self, glib: &TokenStream2) -> TokenStream2 {
        let method_name = format_ident!("pspec_{}", self.name_ident());
        quote! { fn #method_name(&self) -> &'static #glib::ParamSpec }
    }
    pub fn pspec_definition(
        &self,
        index: usize,
        trait_name: &TokenStream2,
        glib: &TokenStream2,
    ) -> TokenStream2 {
        let proto = self.pspec_prototype(glib);
        quote! {
            #proto {
                &<<Self as #glib::object::ObjectSubclassIs>::Subclass as #trait_name>::properties()[#index]
            }
        }
    }
    pub fn notify_prototype(&self) -> TokenStream2 {
        let method_name = format_ident!("notify_{}", self.name_ident());
        quote! { fn #method_name(&self) }
    }
    pub fn notify_definition(
        &self,
        index: usize,
        trait_name: &TokenStream2,
        glib: &TokenStream2,
    ) -> TokenStream2 {
        let proto = self.notify_prototype();
        quote! {
            #proto {
                self.notify_by_pspec(&<<Self as #glib::object::ObjectSubclassIs>::Subclass as #trait_name>::properties()[#index]);
            }
        }
    }
    pub fn connect_prototype(&self, glib: &TokenStream2) -> TokenStream2 {
        let method_name = format_ident!("connect_{}_notify", self.name_ident());
        quote! {
            fn #method_name<F: Fn(&Self) + 'static>(&self, f: F) -> #glib::SignalHandlerId
        }
    }
    pub fn connect_definition(&self, glib: &TokenStream2) -> TokenStream2 {
        let proto = self.connect_prototype(glib);
        let name = self.name.as_ref().unwrap();
        quote! {
            #proto {
                self.connect_notify_local(
                    Some(#name),
                    move |recv, _| f(recv),
                )
            }
        }
    }
    fn new(field: &syn::Field, pod: bool) -> Self {
        Self {
            field: None,
            public: !matches!(&field.vis, syn::Visibility::Inherited),
            skip: !pod,
            type_: PropertyType::Unspecified(field.ty.clone()),
            virtual_: None,
            nullable: None,
            get: pod.then(|| None),
            set: pod.then(|| None),
            name: None,
            nick: None,
            blurb: None,
            minimum: None,
            maximum: None,
            default: None,
            flags: PropertyFlags::empty(),
        }
    }
    pub fn parse(mut field: syn::Field, pod: bool) -> syn::Result<Self> {
        let attr_pos = field.attrs.iter().position(|f| f.path.is_ident("property"));
        let mut prop = if let Some(pos) = attr_pos {
            let attr = field.attrs.remove(pos);
            syn::parse::Parser::parse2(
                super::constrain(|item| Self::parse_attr(item, &field, pod)),
                attr.tokens,
            )?
        } else {
            Self::new(&field, pod)
        };
        if !prop.skip {
            match field.vis {
                syn::Visibility::Inherited | syn::Visibility::Public(_) => {}
                vis => {
                    return Err(syn::Error::new_spanned(
                        vis,
                        "Only `pub` or private is allowed for property visibility",
                    ))
                }
            };
            let name = prop
                .name
                .get_or_insert_with(|| field.ident.as_ref().unwrap().to_string().to_kebab_case());
            prop.nick.get_or_insert_with(|| name.clone());
            prop.blurb.get_or_insert_with(|| name.clone());
            if let Some(nullable) = &prop.nullable {
                if !matches!(
                    &prop.type_,
                    PropertyType::Object(_) | PropertyType::Boxed(_) | PropertyType::Variant(_)
                ) {
                    return Err(syn::Error::new_spanned(nullable, "`nullable` only allowed on properties of type `object`, `boxed`, or `variant`"));
                }
            }
            if let Some(PropertyVirtual::Virtual(virtual_kw)) = &prop.virtual_ {
                if matches!(prop.get, Some(None)) {
                    if pod {
                        return Err(syn::Error::new_spanned(
                            virtual_kw,
                            "custom getter or `!get` required for virtual property",
                        ));
                    } else {
                        return Err(syn::Error::new_spanned(
                            virtual_kw,
                            "custom getter required for readable virtual property",
                        ));
                    }
                }
                if matches!(prop.set, Some(None)) {
                    if pod {
                        return Err(syn::Error::new_spanned(
                            virtual_kw,
                            "custom setter or `!set` required for virtual property",
                        ));
                    } else {
                        return Err(syn::Error::new_spanned(
                            virtual_kw,
                            "custom setter required for writable virtual property",
                        ));
                    }
                }
            }
        }
        if prop.virtual_.is_none() {
            prop.field = Some(field);
        }
        Ok(prop)
    }
    fn parse_attr(
        input: syn::parse::ParseStream,
        field: &syn::Field,
        pod: bool,
    ) -> syn::Result<Self> {
        let mut prop = Self::new(field, pod);
        let mut begin = true;
        prop.skip = false;
        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if begin && pod && lookahead.peek(keywords::skip) {
                input.parse::<keywords::skip>()?;
                if !input.is_empty() {
                    return Err(syn::Error::new(input.span(), "Extra token(s) after `skip`"));
                }
                prop.skip = true;
            } else if lookahead.peek(keywords::get) {
                let kw = input.parse::<keywords::get>()?;
                if (!pod && prop.get.is_some()) || (pod && !matches!(&prop.get, Some(None))) {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `get` attribute"));
                }
                if pod || input.peek(Token![=]) {
                    input.parse::<Token![=]>()?;
                    prop.get.replace(Some(input.parse()?));
                } else {
                    prop.get.replace(None);
                }
            } else if lookahead.peek(keywords::set) {
                let kw = input.parse::<keywords::set>()?;
                if (!pod && prop.set.is_some()) || (pod && !matches!(&prop.set, Some(None))) {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `set` attribute"));
                }
                if pod || input.peek(Token![=]) {
                    input.parse::<Token![=]>()?;
                    prop.set.replace(Some(input.parse()?));
                } else {
                    prop.set.replace(None);
                }
            } else if pod && lookahead.peek(Token![!]) {
                input.parse::<Token![!]>()?;
                let lookahead = input.lookahead1();
                if lookahead.peek(keywords::get) {
                    let kw = input.parse::<keywords::get>()?;
                    if !matches!(&prop.get, Some(None)) {
                        return Err(syn::Error::new_spanned(kw, "Duplicate `get` attribute"));
                    }
                    prop.get.take();
                } else if lookahead.peek(keywords::set) {
                    let kw = input.parse::<keywords::set>()?;
                    if !matches!(&prop.set, Some(None)) {
                        return Err(syn::Error::new_spanned(kw, "Duplicate `set` attribute"));
                    }
                    prop.set.take();
                } else {
                    return Err(lookahead.error());
                }
            } else if lookahead.peek(keywords::name) {
                let kw = input.parse::<keywords::name>()?;
                if prop.name.is_some() {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `name` attribute"));
                }
                input.parse::<Token![=]>()?;
                prop.name.replace(input.parse::<syn::LitStr>()?.value());
            } else if lookahead.peek(keywords::nick) {
                let kw = input.parse::<keywords::nick>()?;
                if prop.nick.is_some() {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `nick` attribute"));
                }
                input.parse::<Token![=]>()?;
                prop.nick.replace(input.parse::<syn::LitStr>()?.value());
            } else if lookahead.peek(keywords::blurb) {
                let kw = input.parse::<keywords::blurb>()?;
                if prop.blurb.is_some() {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `blurb` attribute"));
                }
                input.parse::<Token![=]>()?;
                prop.blurb.replace(input.parse::<syn::LitStr>()?.value());
            } else if lookahead.peek(keywords::minimum) {
                let kw = input.parse::<keywords::minimum>()?;
                if prop.minimum.is_some() {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `minimum` attribute"));
                }
                input.parse::<Token![=]>()?;
                prop.minimum.replace(input.parse::<syn::Lit>()?);
            } else if lookahead.peek(keywords::maximum) {
                let kw = input.parse::<keywords::maximum>()?;
                if prop.maximum.is_some() {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `maximum` attribute"));
                }
                input.parse::<Token![=]>()?;
                prop.maximum.replace(input.parse::<syn::Lit>()?);
            } else if lookahead.peek(keywords::default) {
                let kw = input.parse::<keywords::default>()?;
                if prop.default.is_some() {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `default` attribute"));
                }
                input.parse::<Token![=]>()?;
                prop.default.replace(input.parse::<syn::Lit>()?);
            } else if lookahead.peek(Token![enum]) {
                let kw = input.parse::<Token![enum]>()?;
                if let PropertyType::Unspecified(ty) = std::mem::take(&mut prop.type_) {
                    prop.type_ = PropertyType::Enum(ty);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `enum`, `flags`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::flags) {
                let kw = input.parse::<keywords::flags>()?;
                if let PropertyType::Unspecified(ty) = std::mem::take(&mut prop.type_) {
                    prop.type_ = PropertyType::Flags(ty);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `enum`, `flags`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::boxed) {
                let kw = input.parse::<keywords::boxed>()?;
                if let PropertyType::Unspecified(ty) = std::mem::take(&mut prop.type_) {
                    prop.type_ = PropertyType::Boxed(ty);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `flags`, `enum`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::object) {
                let kw = input.parse::<keywords::object>()?;
                if let PropertyType::Unspecified(ty) = std::mem::take(&mut prop.type_) {
                    prop.type_ = PropertyType::Object(ty);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `flags`, `enum`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::variant) {
                let kw = input.parse::<keywords::variant>()?;
                if let PropertyType::Unspecified(_) = &prop.type_ {
                    if input.peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        let element = input.parse::<syn::LitStr>()?.value();
                        prop.type_ = PropertyType::Variant(Some(element));
                    } else {
                        prop.type_ = PropertyType::Variant(None);
                    }
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `enum`, `flags`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::nullable) {
                let kw = input.parse::<keywords::nullable>()?;
                if prop.nullable.is_some() {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Duplicate `nullable` attribute",
                    ));
                }
                prop.nullable = Some(kw);
            } else if lookahead.peek(Token![virtual]) {
                let kw = input.parse::<Token![virtual]>()?;
                if prop.virtual_.is_some() {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `delegate`, `virtual` is allowed",
                    ));
                }
                prop.virtual_.replace(PropertyVirtual::Virtual(kw));
            } else if lookahead.peek(keywords::delegate) {
                let kw = input.parse::<keywords::delegate>()?;
                if prop.virtual_.is_some() {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `delegate`, `virtual` is allowed",
                    ));
                }
                input.parse::<Token![=]>()?;
                prop.virtual_.replace(PropertyVirtual::Delegate(Box::new(
                    input.parse::<syn::Expr>()?,
                )));
            } else {
                use keywords::*;

                macro_rules! parse_flags {
                    (@body $name:ty: $kw:expr => $flag:expr) => {
                        let kw = input.parse::<$name>()?;
                        let flag = $flag;
                        if prop.flags.contains(flag) {
                            let msg = format!("Duplicate `{}` attribute", <$name as syn::token::CustomToken>::display());
                            return Err(syn::Error::new_spanned(kw, msg));
                        }
                        prop.flags |= flag;
                    };
                    ($name:ty: $kw:expr => $flag:expr) => {
                        if lookahead.peek($kw) {
                            parse_flags!(@body $name: $kw => $flag);
                        } else {
                            return Err(lookahead.error());
                        }
                    };
                    ($name:ty: $kw:expr => $flag:expr, $($names:ty: $kws:expr => $flags:expr),+) => {
                        if lookahead.peek($kw) {
                            parse_flags!(@body $name: $kw => $flag);
                        } else {
                            parse_flags! { $($names: $kws => $flags),+ }
                        }
                    };
                }
                parse_flags! {
                    construct:       construct       => PropertyFlags::CONSTRUCT,
                    construct_only:  construct_only  => PropertyFlags::CONSTRUCT_ONLY,
                    lax_validation:  construct_only  => PropertyFlags::CONSTRUCT_ONLY,
                    explicit_notify: explicit_notify => PropertyFlags::EXPLICIT_NOTIFY,
                    deprecated:      deprecated      => PropertyFlags::DEPRECATED
                }
            }
            begin = false;
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(prop)
    }
}
