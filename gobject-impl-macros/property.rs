use heck::{ToKebabCase, ToSnakeCase};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::Token;

use super::util::*;

mod keywords {
    syn::custom_keyword!(property);

    syn::custom_keyword!(skip);
    syn::custom_keyword!(get);
    syn::custom_keyword!(set);
    syn::custom_keyword!(notify);
    syn::custom_keyword!(connect_notify);

    syn::custom_keyword!(name);
    syn::custom_keyword!(nick);
    syn::custom_keyword!(blurb);
    syn::custom_keyword!(minimum);
    syn::custom_keyword!(maximum);
    syn::custom_keyword!(default);
    syn::custom_keyword!(custom);
    syn::custom_keyword!(flags);

    syn::custom_keyword!(boxed);
    syn::custom_keyword!(object);
    syn::custom_keyword!(variant);
    syn::custom_keyword!(storage);

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
    fn tokens(&self, glib: &TokenStream, readable: bool, writable: bool) -> TokenStream {
        let count = Self::empty().bits().leading_zeros() - Self::all().bits().leading_zeros();
        let mut flags = vec![];
        if readable {
            flags.push(quote! { #glib::ParamFlags::READABLE });
        }
        if writable {
            flags.push(quote! { #glib::ParamFlags::WRITABLE });
        }
        for i in 0..count {
            if let Some(flag) = Self::from_bits(1 << i) {
                if self.contains(flag) {
                    let flag = format!("{:?}", flag);
                    let flag = format_ident!("{}", flag);
                    flags.push(quote! { #glib::ParamFlags::#flag });
                }
            }
        }
        if flags.is_empty() {
            quote! { #glib::ParamFlags::empty() }
        } else {
            quote! { #(#flags)|* }
        }
    }
}

pub enum PropertyType {
    Unspecified,
    Enum(Token![enum]),
    Flags(keywords::flags),
    Boxed(keywords::boxed),
    Object(keywords::object),
    Variant(keywords::variant, syn::LitStr),
}

impl Default for PropertyType {
    fn default() -> Self {
        Self::Unspecified
    }
}

impl PropertyType {
    pub fn span(&self) -> Option<&Span> {
        Some(match self {
            PropertyType::Enum(kw) => &kw.span,
            PropertyType::Flags(kw) => &kw.span,
            PropertyType::Boxed(kw) => &kw.span,
            PropertyType::Object(kw) => &kw.span,
            PropertyType::Variant(kw, _) => &kw.span,
            _ => return None,
        })
    }
}

pub enum PropertyStorage {
    Field(syn::Ident),
    Interface,
    Virtual(Token![virtual]),
    Delegate(Box<syn::Expr>),
}

pub enum PropertyName {
    Field(syn::Ident),
    Custom(syn::LitStr),
}

pub struct Property {
    pub skip: bool,
    pub ty: syn::Type,
    pub special_type: PropertyType,
    pub storage: PropertyStorage,
    pub override_: Option<syn::Type>,
    pub get: Option<Option<syn::Path>>,
    pub set: Option<Option<syn::Path>>,
    pub notify: bool,
    pub connect_notify: bool,
    pub name: PropertyName,
    pub nick: Option<syn::LitStr>,
    pub blurb: Option<syn::LitStr>,
    pub buildable_props: Vec<(syn::Ident, syn::Lit)>,
    pub flags: PropertyFlags,
    pub flag_spans: Vec<Span>,
}

impl Property {
    pub fn from_struct(item: &mut syn::ItemStruct, pod: bool, iface: bool) -> syn::Result<Vec<Self>> {
        let mut named = match &mut item.fields {
            syn::Fields::Named(fields) => fields,
            f => return Err(syn::Error::new_spanned(
                f,
                "struct must have named fields",
            ))
        };

        let mut fields = std::mem::take(&mut named.named).into_iter().collect::<Vec<_>>();

        let mut names = HashSet::new();
        let mut properties = vec![];
        let mut field_index = 0;
        loop {
            if field_index >= fields.len() {
                break;
            }
            let mut remove = false;
            {
                let field = fields[field_index].clone();
                let prop = Self::parse(field, pod, iface)?;
                if !prop.skip {
                    if !matches!(prop.storage, PropertyStorage::Field(_)) {
                        remove = true;
                    }
                    let name = prop.name();
                    if names.contains(&name) {
                        return Err(syn::Error::new(
                            prop.name_span(),
                            format!("Duplicate definition for property `{}`", name),
                        ));
                    }
                    names.insert(name);
                    properties.push(prop);
                }
            }
            if remove {
                fields.remove(field_index);
            } else {
                field_index += 1;
            }
        }
        named.named = fields.into_iter().collect();
        Ok(properties)
    }
    fn new(field: &syn::Field, pod: bool, iface: bool) -> Self {
        let storage = if iface {
            PropertyStorage::Interface
        } else {
            PropertyStorage::Field(field.ident.clone().expect("no field ident"))
        };
        Self {
            skip: !pod,
            ty: field.ty.clone(),
            special_type: PropertyType::Unspecified,
            storage,
            override_: None,
            get: pod.then(|| None),
            set: pod.then(|| None),
            notify: true,
            connect_notify: true,
            name: PropertyName::Field(field.ident.clone().expect("no field ident")),
            nick: None,
            blurb: None,
            buildable_props: vec![],
            flags: PropertyFlags::empty(),
            flag_spans: vec![],
        }
    }
    fn parse(mut field: syn::Field, pod: bool, iface: bool) -> syn::Result<Self> {
        let attr_pos = field.attrs.iter().position(|f| f.path.is_ident("property"));
        let prop = if let Some(pos) = attr_pos {
            let attr = field.attrs.remove(pos);
            syn::parse::Parser::parse2(
                constrain(|item| Self::parse_attr(item, &field, pod, iface)),
                attr.tokens,
            )?
        } else {
            Self::new(&field, pod, iface)
        };
        if prop.get.is_none() && prop.set.is_none() {
            return Err(syn::Error::new_spanned(
                field.ident.as_ref().expect("no field ident"),
                "Property must have at least one of `get` and `set`",
            ));
        }
        Ok(prop)
    }
    fn parse_attr(
        stream: syn::parse::ParseStream,
        field: &syn::Field,
        pod: bool,
        iface: bool,
    ) -> syn::Result<Self> {
        let mut prop = Self::new(field, pod, iface);
        let mut first = true;
        prop.skip = false;
        if stream.is_empty() {
            return Ok(prop);
        }
        let input;
        syn::parenthesized!(input in stream);
        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if first && pod && !iface && lookahead.peek(keywords::skip) {
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
                    if iface {
                        let token = input.parse::<Token![_]>()?;
                        prop.get.replace(Some(syn::parse2(quote! { #token })?));
                    } else {
                        prop.get.replace(Some(input.parse()?));
                    }
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
                    if iface {
                        let token = input.parse::<Token![_]>()?;
                        prop.set.replace(Some(syn::parse2(quote! { #token })?));
                    } else {
                        prop.set.replace(Some(input.parse()?));
                    }
                } else {
                    prop.set.replace(None);
                }
            } else if lookahead.peek(Token![!]) {
                input.parse::<Token![!]>()?;
                let lookahead = input.lookahead1();
                if pod && lookahead.peek(keywords::get) {
                    let kw = input.parse::<keywords::get>()?;
                    if !matches!(&prop.get, Some(None)) {
                        return Err(syn::Error::new_spanned(kw, "Duplicate `get` attribute"));
                    }
                    prop.get.take();
                } else if pod && lookahead.peek(keywords::set) {
                    let kw = input.parse::<keywords::set>()?;
                    if !matches!(&prop.set, Some(None)) {
                        return Err(syn::Error::new_spanned(kw, "Duplicate `set` attribute"));
                    }
                    prop.set.take();
                } else if lookahead.peek(keywords::notify) {
                    let kw = input.parse::<keywords::notify>()?;
                    if !prop.notify {
                        return Err(syn::Error::new_spanned(kw, "Duplicate `notify` attribute"));
                    }
                    prop.notify = false;
                } else if lookahead.peek(keywords::connect_notify) {
                    let kw = input.parse::<keywords::connect_notify>()?;
                    if !prop.connect_notify {
                        return Err(syn::Error::new_spanned(kw, "Duplicate `connect_notify` attribute"));
                    }
                    prop.connect_notify = false;
                } else {
                    return Err(lookahead.error());
                }
            } else if lookahead.peek(keywords::name) {
                let kw = input.parse::<keywords::name>()?;
                if matches!(prop.name, PropertyName::Custom(_)) {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `name` attribute"));
                }
                input.parse::<Token![=]>()?;
                prop.name = PropertyName::Custom(input.parse::<syn::LitStr>()?);
            } else if lookahead.peek(keywords::nick) {
                let kw = input.parse::<keywords::nick>()?;
                if prop.nick.is_some() {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `nick` attribute"));
                }
                input.parse::<Token![=]>()?;
                prop.nick.replace(input.parse::<syn::LitStr>()?);
            } else if lookahead.peek(keywords::blurb) {
                let kw = input.parse::<keywords::blurb>()?;
                if prop.blurb.is_some() {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `blurb` attribute"));
                }
                input.parse::<Token![=]>()?;
                prop.blurb.replace(input.parse::<syn::LitStr>()?);
            } else if lookahead.peek(keywords::minimum) {
                let kw = input.parse::<keywords::minimum>()?;
                let ident = format_ident!("minimum");
                if prop
                    .buildable_props
                    .iter()
                    .any(|(n, _)| *n == ident)
                {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `minimum` attribute"));
                }
                input.parse::<Token![=]>()?;
                prop.buildable_props
                    .push((ident, input.parse::<syn::Lit>()?));
            } else if lookahead.peek(keywords::maximum) {
                let kw = input.parse::<keywords::maximum>()?;
                let ident = format_ident!("maximum");
                if prop
                    .buildable_props
                    .iter()
                    .any(|(n, _)| *n == ident)
                {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `maximum` attribute"));
                }
                input.parse::<Token![=]>()?;
                prop.buildable_props
                    .push((ident, input.parse::<syn::Lit>()?));
            } else if lookahead.peek(keywords::default) {
                let kw = input.parse::<keywords::default>()?;
                let ident = format_ident!("default");
                if prop
                    .buildable_props
                    .iter()
                    .any(|(n, _)| *n == ident)
                {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `default` attribute"));
                }
                input.parse::<Token![=]>()?;
                prop.buildable_props
                    .push((ident, input.parse::<syn::Lit>()?));
            } else if lookahead.peek(keywords::custom) {
                input.parse::<keywords::custom>()?;
                let custom;
                syn::parenthesized!(custom in input);
                while custom.is_empty() {
                    let ident = custom.parse()?;
                    if prop
                        .buildable_props
                        .iter()
                        .any(|(n, _)| *n == ident)
                    {
                        return Err(syn::Error::new_spanned(
                            &ident,
                            format!("Duplicate `{}` attribute", ident),
                        ));
                    }
                    custom.parse::<Token![=]>()?;
                    let value = custom.parse()?;
                    if !custom.is_empty() {
                        custom.parse::<Token![,]>()?;
                    }
                    prop.buildable_props.push((ident, value));
                }
            } else if lookahead.peek(Token![enum]) {
                let kw = input.parse::<Token![enum]>()?;
                if matches!(prop.special_type, PropertyType::Unspecified) {
                    prop.special_type = PropertyType::Enum(kw);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `enum`, `flags`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::flags) {
                let kw = input.parse::<keywords::flags>()?;
                if matches!(prop.special_type, PropertyType::Unspecified) {
                    prop.special_type = PropertyType::Flags(kw);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `enum`, `flags`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::boxed) {
                let kw = input.parse::<keywords::boxed>()?;
                if matches!(prop.special_type, PropertyType::Unspecified) {
                    prop.special_type = PropertyType::Boxed(kw);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `flags`, `enum`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::object) {
                let kw = input.parse::<keywords::object>()?;
                if matches!(prop.special_type, PropertyType::Unspecified) {
                    prop.special_type = PropertyType::Object(kw);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `flags`, `enum`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::variant) {
                let kw = input.parse::<keywords::variant>()?;
                if matches!(prop.special_type, PropertyType::Unspecified) {
                    input.parse::<Token![=]>()?;
                    let element = input.parse::<syn::LitStr>()?;
                    prop.special_type = PropertyType::Variant(kw, element);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `enum`, `flags`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if !iface && lookahead.peek(Token![override]) {
                let kw = input.parse::<Token![override]>()?;
                if prop.override_.is_some() {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Duplicate `override` attribute",
                    ));
                }
                input.parse::<Token![=]>()?;
                prop.override_.replace(input.parse()?);
            } else if !iface && lookahead.peek(Token![virtual]) {
                let kw = input.parse::<Token![virtual]>()?;
                if !matches!(prop.storage, PropertyStorage::Field(_)) {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `storage`, `virtual` is allowed",
                    ));
                }
                prop.storage = PropertyStorage::Virtual(kw);
            } else if !iface && lookahead.peek(keywords::storage) {
                let kw = input.parse::<keywords::storage>()?;
                if !matches!(prop.storage, PropertyStorage::Field(_)) {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `storage`, `virtual` is allowed",
                    ));
                }
                input.parse::<Token![=]>()?;
                prop.storage = PropertyStorage::Delegate(Box::new(input.parse::<syn::Expr>()?));
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
                        prop.flag_spans.push(kw.span);
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
            first = false;
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        // validation
        let name = prop.name();
        if !is_valid_name(&name) {
            return Err(syn::Error::new(
                prop.name_span(),
                format!("Invalid property name '{}'. Property names must start with an ASCII letter and only contain ASCII letters, numbers, '-' or '_'", name),
            ));
        }
        if prop.override_.is_some() {
            if let Some(nick) = &prop.nick {
                return Err(syn::Error::new_spanned(
                    nick,
                    "`nick` not allowed on override property",
                ));
            }
            if let Some(blurb) = &prop.blurb {
                return Err(syn::Error::new_spanned(
                    blurb,
                    "`blurb` not allowed on override property",
                ));
            }
            if let Some((ident, _)) = prop.buildable_props.first() {
                return Err(syn::Error::new_spanned(
                    ident,
                    format!("`{}` not allowed on override property", ident),
                ));
            }
            if let Some(flag) = prop.flag_spans.first() {
                return Err(syn::Error::new(
                    *flag,
                    "flag not allowed on override property",
                ));
            }
            if let Some(span) = prop.special_type.span() {
                return Err(syn::Error::new(
                    *span,
                    "type specifier not allowed on override property",
                ));
            }
        }
        if let PropertyStorage::Virtual(virtual_kw) = &prop.storage {
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

        Ok(prop)
    }
    pub fn create(&self, go: &syn::Ident) -> TokenStream {
        let glib = quote! { #go::glib };
        let name = self.name();
        if let Some(iface) = &self.override_ {
            return quote! {
                #glib::ParamSpecOverride::for_interface::<#iface>(#name)
            };
        }
        let nick = self
            .nick
            .as_ref()
            .map(|s| s.value())
            .unwrap_or_else(|| name.clone());
        let blurb = self
            .blurb
            .as_ref()
            .map(|s| s.value())
            .unwrap_or_else(|| name.clone());
        let flags = self
            .flags
            .tokens(&glib, self.get.is_some(), self.set.is_some());
        let ty = &self.ty;
        let static_type = quote! {
            <<<#ty as #go::ParamStore>::Type as #glib::value::ValueType>::Type as #glib::StaticType>::static_type(),
        };
        let props = self
            .buildable_props
            .iter()
            .map(|(ident, value)| quote! { .#ident(#value) });
        let builder = match &self.special_type {
            PropertyType::Enum(_) => quote! {
                <#go::ParamSpecEnumBuilder as ::core::default::Default>::default()
            },
            PropertyType::Flags(_) => quote! {
                <#go::ParamSpecFlagsBuilder as ::core::default::Default>::default()
            },
            PropertyType::Boxed(_) => quote! {
                <#go::ParamSpecBoxedBuilder as ::core::default::Default>::default()
            },
            PropertyType::Object(_) => quote! {
                <#go::ParamSpecObjectBuilder as ::core::default::Default>::default()
            },
            _ => quote! { <#ty as #go::ParamSpecBuildable>::builder() },
        };
        let type_prop = match &self.special_type {
            PropertyType::Unspecified => None,
            PropertyType::Variant(_, element) => Some(quote! { .type_(#element) }),
            _ => Some(quote! { .type_(#static_type) }),
        };
        quote! {
            #builder
            #type_prop
            #(#props)*
            .build(#name, #nick, #blurb, #flags)
        }
    }
    pub fn name(&self) -> String {
        match &self.name {
            PropertyName::Field(name) => name.to_string().to_kebab_case(),
            PropertyName::Custom(name) => name.value(),
        }
    }
    pub fn name_span(&self) -> Span {
        match &self.name {
            PropertyName::Field(name) => name.span(),
            PropertyName::Custom(name) => name.span(),
        }
    }
    fn inner_type(&self, go: &syn::Ident) -> TokenStream {
        let ty = &self.ty;
        quote! { <#ty as #go::ParamStore>::Type }
    }
    fn is_interface(&self) -> bool {
        matches!(self.storage, PropertyStorage::Interface)
    }
    fn field_storage(&self, go: Option<&syn::Ident>) -> TokenStream {
        match &self.storage {
            PropertyStorage::Field(field) => {
                let recv = if let Some(go) = go {
                    quote! { #go::glib::subclass::prelude::ObjectSubclassIsExt::imp(self) }
                } else {
                    quote! { self }
                };
                quote! { #recv.#field }
            }
            PropertyStorage::Delegate(delegate) => quote! { #delegate },
            _ => unreachable!("cannot get storage for interface/virtual property"),
        }
    }
    #[inline]
    fn method_call(
        method_name: &syn::Ident,
        args: TokenStream,
        trait_name: &syn::Ident,
        glib: &TokenStream,
    ) -> TokenStream {
        quote! {
            <<Self as #glib::subclass::types::ObjectSubclass>::Type as #trait_name>::#method_name(obj, #args)
        }
    }
    pub fn get_impl(
        &self,
        index: usize,
        trait_name: Option<&syn::Ident>,
        go: &syn::Ident,
    ) -> Option<TokenStream> {
        if self.is_interface() {
            return None;
        }
        self.get.as_ref().map(|expr| {
            let glib = quote! { #go::glib };
            let expr = if let Some(expr) = expr {
                quote! { #glib::ToValue::to_value(&#expr()) }
            } else if let Some(trait_name) = trait_name {
                let method_name = format_ident!("{}", self.name().to_snake_case());
                let call = Self::method_call(&method_name, quote! {}, trait_name, &glib);
                    quote! { #glib::ToValue::to_value(&#call) }
            } else {
                let field = self.field_storage(None);
                quote! { #go::ParamStoreRead::get_value(&#field) }
            };
            quote! {
                #index => {
                    #expr
                }
            }
        })
    }
    pub fn getter_prototype(&self, go: &syn::Ident) -> Option<TokenStream> {
        matches!(self.get, Some(None)).then(|| {
            let method_name = format_ident!("{}", self.name().to_snake_case());
            let ty = self.inner_type(go);
            quote! { fn #method_name(&self) -> #ty }
        })
    }
    pub fn getter_definition(&self, go: &syn::Ident) -> Option<TokenStream> {
        matches!(self.get, Some(None)).then(|| {
            let proto = self.getter_prototype(go).expect("no proto for getter");
            let body = if self.is_interface() {
                let name = self.name();
                quote! { <Self as #go::glib::object::ObjectExt>::property(self, #name) }
            } else {
                let field = self.field_storage(Some(go));
                let ty = self.inner_type(go);
                quote! { #go::ParamStoreRead::get_value(&#field).get::<#ty>().unwrap() }
            };
            quote! {
                #proto {
                    #body
                }
            }
        })
    }
    pub fn set_impl(
        &self,
        index: usize,
        trait_name: Option<&syn::Ident>,
        go: &syn::Ident,
    ) -> Option<TokenStream> {
        if self.is_interface() {
            return None;
        }
        self.set.as_ref().map(|expr| {
            let ty = self.inner_type(go);
            let set = if let Some(expr) = &expr {
                quote! { #expr(value.get::<#ty>().unwrap()); }
            } else if trait_name.is_some() && self.flags.contains(PropertyFlags::EXPLICIT_NOTIFY) {
                let method_name = format_ident!("set_{}", self.name().to_snake_case());
                let glib = quote! { #go::glib };
                Self::method_call(&method_name, quote! { value }, trait_name.unwrap(), &glib)
            } else {
                let field = self.field_storage(None);
                quote! { #go::ParamStoreWrite::set_value(&#field, &value) }
            };
            quote! { #index => { #set; } }
        })
    }
    pub fn setter_prototype(&self, go: &syn::Ident) -> Option<TokenStream> {
        matches!(self.set, Some(None)).then(|| {
            let method_name = format_ident!("set_{}", self.name().to_snake_case());
            let ty = self.inner_type(go);
            quote! { fn #method_name(&self, value: #ty) }
        })
    }
    pub fn setter_definition(
        &self,
        index: usize,
        properties_path: &TokenStream,
        go: &syn::Ident,
    ) -> Option<TokenStream> {
        matches!(self.set, Some(None)).then(|| {
            let proto = self.setter_prototype(go).expect("no setter proto");
            let body =
                if self.is_interface() || !self.flags.contains(PropertyFlags::EXPLICIT_NOTIFY) {
                    let name = self.name();
                    quote! {
                        <Self as #go::glib::object::ObjectExt>::set_property(self, #name, value);
                    }
                } else {
                    let field = self.field_storage(Some(go));
                    quote! {
                        if #go::ParamStoreWrite::set(&#field, &value) {
                            <Self as #go::glib::object::ObjectExt>::notify_by_pspec(
                                self,
                                &#properties_path()[#index]
                            );
                        }
                    }
                };
            quote! {
                #proto {
                    #body
                }
            }
        })
    }
    pub fn pspec_prototype(&self, glib: &TokenStream) -> TokenStream {
        let method_name = format_ident!("pspec_{}", self.name().to_snake_case());
        quote! { fn #method_name(&self) -> &'static #glib::ParamSpec }
    }
    pub fn pspec_definition(
        &self,
        index: usize,
        properties_path: &TokenStream,
        glib: &TokenStream,
    ) -> TokenStream {
        let proto = self.pspec_prototype(glib);
        quote! {
            #proto { &#properties_path()[#index] }
        }
    }
    pub fn notify_prototype(&self) -> TokenStream {
        let method_name = format_ident!("notify_{}", self.name().to_snake_case());
        quote! { fn #method_name(&self) }
    }
    pub fn notify_definition(
        &self,
        index: usize,
        properties_path: &TokenStream,
        glib: &TokenStream,
    ) -> TokenStream {
        let proto = self.notify_prototype();
        quote! {
            #proto {
                <Self as #glib::object::ObjectExt>::notify_by_pspec(
                    self,
                    &#properties_path()[#index]
                );
            }
        }
    }
    pub fn connect_prototype(&self, glib: &TokenStream) -> TokenStream {
        let method_name = format_ident!("connect_{}_notify", self.name().to_snake_case());
        quote! {
            fn #method_name<F: Fn(&Self) + 'static>(&self, f: F) -> #glib::SignalHandlerId
        }
    }
    pub fn connect_definition(&self, glib: &TokenStream) -> TokenStream {
        let proto = self.connect_prototype(glib);
        let name = self.name();
        quote! {
            #proto {
                <Self as #glib::object::ObjectExt>::connect_notify_local(
                    self,
                    Some(#name),
                    move |recv, _| f(recv),
                )
            }
        }
    }
}
