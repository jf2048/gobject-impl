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
    syn::custom_keyword!(auto); // for use with set
    syn::custom_keyword!(notify_func);
    syn::custom_keyword!(connect_notify_func);

    syn::custom_keyword!(name);
    syn::custom_keyword!(nick);
    syn::custom_keyword!(blurb);
    syn::custom_keyword!(minimum);
    syn::custom_keyword!(maximum);
    syn::custom_keyword!(default);
    syn::custom_keyword!(subtype); // for use with ParamSpecGType
    syn::custom_keyword!(variant); // for use with ParamSpecVariant
    syn::custom_keyword!(custom);
    syn::custom_keyword!(flags);

    syn::custom_keyword!(boxed);
    syn::custom_keyword!(object);
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
    fn from_ident(ident: &syn::Ident) -> Option<Self> {
        Some(match ident.to_string().as_str() {
            "construct" => PropertyFlags::CONSTRUCT,
            "construct_only" => PropertyFlags::CONSTRUCT_ONLY,
            "lax_validation" => PropertyFlags::LAX_VALIDATION,
            "explicit_notify" => PropertyFlags::EXPLICIT_NOTIFY,
            "deprecated" => PropertyFlags::DEPRECATED,
            _ => return None,
        })
    }
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
    Enum(syn::Ident),
    Flags(syn::Ident),
    Boxed(syn::Ident),
    Object(syn::Ident),
}

impl Default for PropertyType {
    fn default() -> Self {
        Self::Unspecified
    }
}

impl PropertyType {
    pub fn ident(&self) -> Option<&syn::Ident> {
        Some(match self {
            PropertyType::Enum(kw) => kw,
            PropertyType::Flags(kw) => kw,
            PropertyType::Boxed(kw) => kw,
            PropertyType::Object(kw) => kw,
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

#[derive(PartialEq)]
pub enum PropertyPermission {
    Deny,
    Allow,
    AllowAuto,
    AllowCustomDefault,
    AllowCustom(syn::Ident),
}

impl PropertyPermission {
    fn default_for(set: bool, pod: bool) -> Self {
        if pod {
            if set {
                Self::AllowAuto
            } else {
                Self::Allow
            }
        } else {
            Self::Deny
        }
    }
    fn is_allowed(&self) -> bool {
        !matches!(self, Self::Deny)
    }
}

pub struct Property {
    pub skip: bool,
    pub ty: syn::Type,
    pub special_type: PropertyType,
    pub storage: PropertyStorage,
    pub override_: Option<syn::Type>,
    pub get: PropertyPermission,
    pub set: PropertyPermission,
    pub no_notify: Option<syn::Ident>,
    pub no_connect_notify: Option<syn::Ident>,
    pub name: PropertyName,
    pub nick: Option<syn::LitStr>,
    pub blurb: Option<syn::LitStr>,
    pub buildable_props: Vec<(syn::Ident, syn::Lit)>,
    pub subtype: Option<syn::Type>,
    pub flags: PropertyFlags,
    pub flag_idents: Vec<syn::Ident>,
}

impl Property {
    pub fn from_struct(
        item: &mut syn::ItemStruct,
        pod: bool,
        iface: bool,
    ) -> syn::Result<Vec<Self>> {
        let mut named = match &mut item.fields {
            syn::Fields::Named(fields) => fields,
            f => return Err(syn::Error::new_spanned(f, "struct must have named fields")),
        };

        let mut fields = std::mem::take(&mut named.named)
            .into_iter()
            .collect::<Vec<_>>();

        let mut names = HashSet::new();
        let mut properties = vec![];
        let mut field_index = 0;
        loop {
            if field_index >= fields.len() {
                break;
            }
            let mut remove = false;
            {
                let prop = Self::parse(&mut fields[field_index], pod, iface)?;
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
            get: PropertyPermission::default_for(false, pod),
            set: PropertyPermission::default_for(true, pod),
            no_notify: None,
            no_connect_notify: None,
            name: PropertyName::Field(field.ident.clone().expect("no field ident")),
            nick: None,
            blurb: None,
            buildable_props: vec![],
            subtype: None,
            flags: PropertyFlags::empty(),
            flag_idents: vec![],
        }
    }
    fn parse(field: &mut syn::Field, pod: bool, iface: bool) -> syn::Result<Self> {
        let attr_pos = field.attrs.iter().position(|f| f.path.is_ident("property"));
        let prop = if let Some(pos) = attr_pos {
            let attr = field.attrs.remove(pos);
            syn::parse::Parser::parse2(
                constrain(|item| Self::parse_attr(item, field, pod, iface)),
                attr.tokens,
            )?
        } else {
            Self::new(field, pod, iface)
        };
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
                if prop.get != PropertyPermission::default_for(false, pod) {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `get` attribute"));
                }
                if !iface && (pod || input.peek(Token![=])) {
                    input.parse::<Token![=]>()?;
                    if input.peek(Token![_]) {
                        input.parse::<Token![_]>()?;
                        prop.get = PropertyPermission::AllowCustomDefault;
                    } else {
                        prop.get = PropertyPermission::AllowCustom(input.parse()?);
                    }
                } else {
                    prop.get = PropertyPermission::Allow;
                }
            } else if lookahead.peek(keywords::set) {
                let kw = input.parse::<keywords::set>()?;
                if prop.set != PropertyPermission::default_for(true, pod) {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `set` attribute"));
                }
                if pod || input.peek(Token![=]) {
                    input.parse::<Token![=]>()?;
                    if !iface && input.peek(Token![_]) {
                        input.parse::<Token![_]>()?;
                        prop.set = PropertyPermission::AllowCustomDefault;
                    } else if iface || input.peek(keywords::auto) {
                        let kw = input.parse::<keywords::auto>()?;
                        if pod {
                            return Err(syn::Error::new_spanned(
                                kw,
                                "unneccesary `set = auto` on `pod` type",
                            ));
                        }
                        prop.set = PropertyPermission::AllowAuto;
                    } else {
                        prop.set = PropertyPermission::AllowCustom(input.parse()?);
                    }
                } else {
                    prop.set = PropertyPermission::Allow;
                }
            } else if lookahead.peek(Token![!]) {
                input.parse::<Token![!]>()?;
                let lookahead = input.lookahead1();
                if pod && lookahead.peek(keywords::get) {
                    let kw = input.parse::<keywords::get>()?;
                    if prop.get != PropertyPermission::default_for(false, pod) {
                        return Err(syn::Error::new_spanned(kw, "Duplicate `get` attribute"));
                    }
                    prop.get = PropertyPermission::Deny;
                } else if pod && lookahead.peek(keywords::set) {
                    let kw = input.parse::<keywords::set>()?;
                    if prop.set != PropertyPermission::default_for(true, pod) {
                        return Err(syn::Error::new_spanned(kw, "Duplicate `set` attribute"));
                    }
                    prop.set = PropertyPermission::Deny;
                } else if lookahead.peek(keywords::notify_func) {
                    let kw = input.call(syn::ext::IdentExt::parse_any)?;
                    if prop.no_notify.is_some() {
                        return Err(syn::Error::new_spanned(kw, "Duplicate `notify` attribute"));
                    }
                    prop.no_notify.replace(kw);
                } else if lookahead.peek(keywords::connect_notify_func) {
                    let kw = input.call(syn::ext::IdentExt::parse_any)?;
                    if prop.no_connect_notify.is_some() {
                        return Err(syn::Error::new_spanned(
                            kw,
                            "Duplicate `connect_notify` attribute",
                        ));
                    }
                    prop.no_connect_notify.replace(kw);
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
                prop.nick.replace(input.parse()?);
            } else if lookahead.peek(keywords::blurb) {
                let kw = input.parse::<keywords::blurb>()?;
                if prop.blurb.is_some() {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `blurb` attribute"));
                }
                input.parse::<Token![=]>()?;
                prop.blurb.replace(input.parse()?);
            } else if lookahead.peek(keywords::minimum)
                || lookahead.peek(keywords::maximum)
                || lookahead.peek(keywords::default)
            {
                let ident = input.call(syn::ext::IdentExt::parse_any)?;
                if prop.buildable_props.iter().any(|(n, _)| *n == ident) {
                    return Err(syn::Error::new_spanned(
                        &ident,
                        format!("Duplicate `{}` attribute", ident),
                    ));
                }
                input.parse::<Token![=]>()?;
                prop.buildable_props.push((ident, input.parse()?));
            } else if lookahead.peek(keywords::subtype) {
                let kw = input.parse::<keywords::subtype>()?;
                if prop.subtype.is_some() {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `subtype` attribute"));
                }
                input.parse::<Token![=]>()?;
                prop.subtype.replace(input.parse()?);
            } else if lookahead.peek(keywords::custom) {
                input.parse::<keywords::custom>()?;
                let custom;
                syn::parenthesized!(custom in input);
                while custom.is_empty() {
                    let ident = custom.call(syn::ext::IdentExt::parse_any)?;
                    if prop.buildable_props.iter().any(|(n, _)| *n == ident) {
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
                let kw = input.call(syn::ext::IdentExt::parse_any)?;
                if matches!(prop.special_type, PropertyType::Unspecified) {
                    prop.special_type = PropertyType::Enum(kw);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `enum`, `flags`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::flags) {
                let kw = input.call(syn::ext::IdentExt::parse_any)?;
                if matches!(prop.special_type, PropertyType::Unspecified) {
                    prop.special_type = PropertyType::Flags(kw);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `enum`, `flags`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::boxed) {
                let kw = input.call(syn::ext::IdentExt::parse_any)?;
                if matches!(prop.special_type, PropertyType::Unspecified) {
                    prop.special_type = PropertyType::Boxed(kw);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `flags`, `enum`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::object) {
                let kw = input.call(syn::ext::IdentExt::parse_any)?;
                if matches!(prop.special_type, PropertyType::Unspecified) {
                    prop.special_type = PropertyType::Object(kw);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `flags`, `enum`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::variant) {
                let ident = input.call(syn::ext::IdentExt::parse_any)?;
                if prop.buildable_props.iter().any(|(n, _)| *n == ident) {
                    return Err(syn::Error::new_spanned(
                        &ident,
                        format!("Duplicate `{}` attribute", ident),
                    ));
                }
                input.parse::<Token![=]>()?;
                let s = input.parse::<syn::LitStr>()?;
                prop.buildable_props.push((ident, syn::Lit::Str(s)));
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
            } else if lookahead.peek(keywords::construct)
                || lookahead.peek(keywords::construct_only)
                || lookahead.peek(keywords::lax_validation)
                || lookahead.peek(keywords::explicit_notify)
                || lookahead.peek(keywords::deprecated)
            {
                let ident: syn::Ident = input.call(syn::ext::IdentExt::parse_any)?;
                let flag = PropertyFlags::from_ident(&ident).unwrap();
                if prop.flags.contains(flag) {
                    let msg = format!("Duplicate `{}` attribute", ident);
                    return Err(syn::Error::new_spanned(&ident, msg));
                }
                prop.flag_idents.push(ident);
                prop.flags |= flag;
            } else {
                return Err(lookahead.error());
            }
            first = false;
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        // validation
        if !prop.skip {
            let name = prop.name();
            if !is_valid_name(&name) {
                return Err(syn::Error::new(prop.name_span(), format!("Invalid property name '{}'. Property names must start with an ASCII letter and only contain ASCII letters, numbers, '-' or '_'", name)));
            }
            if !prop.get.is_allowed() && !prop.set.is_allowed() {
                return Err(syn::Error::new_spanned(
                    field.ident.as_ref().expect("no field ident"),
                    "Property must have at least one of `get` and `set`",
                ));
            }
            if let PropertyPermission::AllowCustom(method) = &prop.get {
                if method == &prop.getter_name() {
                    return Err(syn::Error::new_spanned(
                        method,
                        "custom getter name conflicts with trait method",
                    ));
                }
            }
            if let PropertyPermission::AllowCustom(method) = &prop.set {
                if method == &prop.setter_name() {
                    return Err(syn::Error::new_spanned(
                        method,
                        "custom setter name conflicts with trait method",
                    ));
                }
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
                for (ident, _) in &prop.buildable_props {
                    if ident != "minimum" && ident != "maximum" {
                        return Err(syn::Error::new_spanned(
                            ident,
                            format!("`{}` not allowed on override property", ident),
                        ));
                    }
                }
                if let Some(flag) = prop.flag_idents.first() {
                    return Err(syn::Error::new_spanned(
                        &flag,
                        format!("`{}` not allowed on override property", flag),
                    ));
                }
                if let Some(ident) = prop.special_type.ident() {
                    return Err(syn::Error::new_spanned(
                        ident,
                        "type specifier not allowed on override property",
                    ));
                }
                if let Some(notify) = &prop.no_notify {
                    return Err(syn::Error::new_spanned(
                        notify,
                        "`notify` not allowed on override property",
                    ));
                }
                if let Some(connect_notify) = &prop.no_connect_notify {
                    return Err(syn::Error::new_spanned(
                        connect_notify,
                        "`connect_notify` not allowed on override property",
                    ));
                }
            }
            if matches!(prop.set, PropertyPermission::AllowAuto) {
                for flag in &prop.flag_idents {
                    if flag == "explicit_notify" || flag == "lax_validation" {
                        return Err(syn::Error::new_spanned(
                            &flag,
                            format!("`{}` unnecessary when using `set = auto`", flag),
                        ));
                    }
                }
                prop.flags |= PropertyFlags::EXPLICIT_NOTIFY | PropertyFlags::LAX_VALIDATION;
            }
            if matches!(prop.storage, PropertyStorage::Virtual(_)) {
                if matches!(prop.get, PropertyPermission::Allow) {
                    prop.get = PropertyPermission::AllowCustomDefault;
                }
                if matches!(prop.set, PropertyPermission::Allow) {
                    prop.set = PropertyPermission::AllowCustomDefault;
                }
            }
            if matches!(prop.get, PropertyPermission::AllowCustomDefault) {
                let ident = format_ident!("_{}", prop.name().to_snake_case());
                prop.get = PropertyPermission::AllowCustom(ident);
            }
            if matches!(prop.set, PropertyPermission::AllowCustomDefault) {
                let ident = format_ident!("_set_{}", prop.name().to_snake_case());
                prop.set = PropertyPermission::AllowCustom(ident);
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
            .tokens(&glib, self.get.is_allowed(), self.set.is_allowed());
        let ty = self.inner_type(go);
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
            _ => Some(quote! {
                .type_::<<#ty as #glib::value::ValueType>::Type>()
            }),
        };
        let subtype_prop = self.subtype.as_ref().map(|subtype| {
            quote! {
                .subtype::<<#subtype as #glib::value::ValueType>::Type>()
            }
        });
        quote! {
            #builder
            #type_prop
            #subtype_prop
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
    fn is_object(&self) -> bool {
        !matches!(self.storage, PropertyStorage::Interface)
    }
    fn inner_type(&self, go: &syn::Ident) -> TokenStream {
        let ty = &self.ty;
        if self.is_object() && !matches!(self.storage, PropertyStorage::Virtual(_)) {
            quote! { <#ty as #go::ParamStore>::Type }
        } else {
            quote! { #ty }
        }
    }
    fn field_storage(&self, object_type: Option<&TokenStream>, go: &syn::Ident) -> TokenStream {
        let recv = if let Some(object_type) = object_type {
            quote! {
                #go::glib::subclass::prelude::ObjectSubclassIsExt::imp(
                    self.upcast_ref::<#object_type>()
                )
            }
        } else {
            quote! { self }
        };
        match &self.storage {
            PropertyStorage::Field(field) => quote! { #recv.#field },
            PropertyStorage::Delegate(delegate) => quote! { #recv.#delegate },
            _ => unreachable!("cannot get storage for interface/virtual property"),
        }
    }
    fn find_buildable_prop(&self, name: &str) -> Option<&syn::Lit> {
        self.buildable_props
            .iter()
            .find_map(|(i, l)| (i == name).then(|| l))
    }
    #[inline]
    fn getter_name(&self) -> syn::Ident {
        format_ident!("{}", self.name().to_snake_case())
    }
    pub fn get_impl(
        &self,
        index: usize,
        inheritance: &ClassInheritance,
        go: &syn::Ident,
    ) -> Option<TokenStream> {
        if !self.is_object() {
            return None;
        }
        self.get.is_allowed().then(|| {
            let glib = quote! { #go::glib };
            let body = if self.override_.is_none() {
                let method_name = self.getter_name();
                let recv_ty = match inheritance {
                    ClassInheritance::Abstract(trait_name) => quote! {
                        <<Self as #glib::subclass::types::ObjectSubclass>::Type as #trait_name>
                    },
                    ClassInheritance::Final => quote! {
                        <Self as #glib::subclass::types::ObjectSubclass>::Type
                    },
                };
                quote! {
                    #glib::ToValue::to_value(&#recv_ty::#method_name(obj))
                }
            } else if let PropertyPermission::AllowCustom(method) = &self.get {
                quote! { #glib::ToValue::to_value(&obj.#method()) }
            } else {
                let field = self.field_storage(None, go);
                quote! { #go::ParamStoreRead::get_value(&#field) }
            };
            quote! {
                #index => {
                    #body
                }
            }
        })
    }
    pub fn getter_prototype(&self, go: &syn::Ident) -> Option<TokenStream> {
        if self.override_.is_some() {
            return None;
        }
        self.get.is_allowed().then(|| {
            let method_name = self.getter_name();
            let ty = self.inner_type(go);
            quote! { fn #method_name(&self) -> #ty }
        })
    }
    pub fn getter_definition(
        &self,
        object_type: &TokenStream,
        go: &syn::Ident,
    ) -> Option<TokenStream> {
        self.getter_prototype(go).map(|proto| {
            let body = if let PropertyPermission::AllowCustom(method) = &self.get {
                quote! { self.upcast_ref::<#object_type>().#method() }
            } else if self.is_object() {
                let field = self.field_storage(Some(object_type), go);
                quote! { #go::ParamStoreRead::get(&#field) }
            } else {
                let name = self.name();
                quote! { <Self as #go::glib::object::ObjectExt>::property(self, #name) }
            };
            quote! {
                #[inline]
                #proto {
                    #body
                }
            }
        })
    }
    #[inline]
    fn setter_name(&self) -> syn::Ident {
        format_ident!("set_{}", self.name().to_snake_case())
    }
    #[inline]
    fn can_inline_set(&self) -> bool {
        self.flags
            .contains(PropertyFlags::EXPLICIT_NOTIFY | PropertyFlags::LAX_VALIDATION)
    }
    fn setter_validations(&self) -> Option<TokenStream> {
        if !self.flags.contains(PropertyFlags::LAX_VALIDATION) {
            return None;
        }
        let min = self
            .find_buildable_prop("minimum")
            .map(|min| quote! { assert!(value >= #min); });
        let max = self
            .find_buildable_prop("maximum")
            .map(|max| quote! { assert!(value <= #max); });
        Some(quote! {
            #min
            #max
        })
    }
    pub fn set_impl(
        &self,
        index: usize,
        inheritance: &ClassInheritance,
        go: &syn::Ident,
    ) -> Option<TokenStream> {
        if !self.is_object() {
            return None;
        }
        self.set.is_allowed().then(|| {
            let glib = quote! { #go::glib };
            let can_inline = self.can_inline_set();
            let construct_only = self.flags.contains(PropertyFlags::CONSTRUCT_ONLY);
            let is_override = self.override_.is_some();
            let body = match (can_inline, construct_only, is_override) {
                (true, false, false) => {
                    let method_name = self.setter_name();
                    let recv_ty = match inheritance {
                        ClassInheritance::Abstract(trait_name) => quote! {
                            <<Self as #glib::subclass::types::ObjectSubclass>::Type as #trait_name>
                        },
                        ClassInheritance::Final => quote! {
                            <Self as #glib::subclass::types::ObjectSubclass>::Type
                        }
                    };
                    quote! { #recv_ty::#method_name(obj, value.get_owned().unwrap()); }
                }
                _ => {
                    let ty = self.inner_type(go);
                    match &self.set {
                        PropertyPermission::AllowCustom(method) => quote! {
                            obj.#method(value.get_owned::<#ty>().unwrap());
                        },
                        PropertyPermission::AllowAuto => {
                            let field = self.field_storage(None, go);
                            let validations = self.setter_validations();
                            let set = if self.get.is_allowed() && self.flags.contains(PropertyFlags::EXPLICIT_NOTIFY) {
                                quote! {
                                    if #go::ParamStoreWriteChanged::set_checked(&#field, value) {
                                        <<Self as #glib::subclass::types::ObjectSubclass>::Type as #glib::object::ObjectExt>::notify_by_pspec(
                                            obj,
                                            pspec
                                        );
                                    }
                                }
                            } else {
                                quote! {
                                    #go::ParamStoreWrite::set_value(&#field, &value);
                                }
                            };
                            quote! {
                                let value = value.get_owned::<#ty>().unwrap();
                                #validations
                                #set
                            }
                        },
                        PropertyPermission::Allow => {
                            let field = self.field_storage(None, go);
                            quote! {
                                #go::ParamStoreWrite::set_value(&#field, &value);
                            }
                        },
                        _ => unreachable!()
                    }
                }
            };
            quote! { #index => { #body } }
        })
    }
    pub fn setter_prototype(&self, go: &syn::Ident) -> Option<TokenStream> {
        if self.override_.is_some() || self.flags.contains(PropertyFlags::CONSTRUCT_ONLY) {
            return None;
        }
        self.set.is_allowed().then(|| {
            let method_name = self.setter_name();
            let ty = self.inner_type(go);
            quote! { fn #method_name(&self, value: #ty) }
        })
    }
    pub fn setter_definition(
        &self,
        index: usize,
        object_type: &TokenStream,
        properties_path: &TokenStream,
        go: &syn::Ident,
    ) -> Option<TokenStream> {
        self.setter_prototype(go).map(|proto| {
            let body = match (self.is_object(), self.can_inline_set()) {
                (true, true) => match &self.set {
                    PropertyPermission::AllowCustom(method) => quote! {
                        self.upcast_ref::<#object_type>().#method(value);
                    },
                    PropertyPermission::AllowAuto => {
                        let field = self.field_storage(Some(object_type), go);
                        let validations = self.setter_validations();
                        let set = if self.get.is_allowed() {
                            quote! {
                                if #go::ParamStoreWriteChanged::set_checked(&#field, value) {
                                    <Self as #go::glib::object::ObjectExt>::notify_by_pspec(
                                        self,
                                        &#properties_path()[#index]
                                    );
                                }
                            }
                        } else {
                            quote! {
                                #go::ParamStoreWriteChanged::set(&#field, value);
                            }
                        };
                        quote! {
                            #validations
                            #set
                        }
                    }
                    PropertyPermission::Allow => {
                        let field = self.field_storage(Some(object_type), go);
                        quote! {
                            #go::ParamStoreWriteChanged::set(&#field, value);
                        }
                    }
                    _ => unreachable!(),
                },
                _ => {
                    let name = self.name();
                    quote! {
                        <Self as #go::glib::object::ObjectExt>::set_property(self, #name, value);
                    }
                }
            };
            quote! {
                #[inline]
                #proto {
                    #body
                }
            }
        })
    }
    pub fn pspec_prototype(&self, glib: &TokenStream) -> Option<TokenStream> {
        let method_name = format_ident!("pspec_{}", self.name().to_snake_case());
        Some(quote! { fn #method_name() -> &'static #glib::ParamSpec })
    }
    pub fn pspec_definition(
        &self,
        index: usize,
        properties_path: &TokenStream,
        glib: &TokenStream,
    ) -> Option<TokenStream> {
        self.pspec_prototype(glib).map(|proto| {
            quote! {
                #[inline]
                #proto { &#properties_path()[#index] }
            }
        })
    }
    pub fn notify_prototype(&self) -> Option<TokenStream> {
        if self.override_.is_some() || self.no_notify.is_some() {
            return None;
        }
        let method_name = format_ident!("notify_{}", self.name().to_snake_case());
        Some(quote! { fn #method_name(&self) })
    }
    pub fn notify_definition(
        &self,
        index: usize,
        properties_path: &TokenStream,
        glib: &TokenStream,
    ) -> Option<TokenStream> {
        self.notify_prototype().map(|proto| {
            quote! {
                #[inline]
                #proto {
                    <Self as #glib::object::ObjectExt>::notify_by_pspec(
                        self,
                        &#properties_path()[#index]
                    );
                }
            }
        })
    }
    pub fn connect_prototype(&self, glib: &TokenStream) -> Option<TokenStream> {
        if self.override_.is_some() || self.no_connect_notify.is_some() {
            return None;
        }
        let method_name = format_ident!("connect_{}_notify", self.name().to_snake_case());
        Some(quote! {
            fn #method_name<F: Fn(&Self) + 'static>(&self, f: F) -> #glib::SignalHandlerId
        })
    }
    pub fn connect_definition(&self, glib: &TokenStream) -> Option<TokenStream> {
        self.connect_prototype(glib).map(|proto| {
            let name = self.name();
            quote! {
                #[inline]
                #proto {
                    <Self as #glib::object::ObjectExt>::connect_notify_local(
                        self,
                        Some(#name),
                        move |recv, _| f(recv),
                    )
                }
            }
        })
    }
}
