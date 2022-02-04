use heck::{ToKebabCase, ToSnakeCase};
use proc_macro2::{TokenStream, Span};
use quote::{format_ident, quote, ToTokens};
use syn::Token;

mod keywords {
    syn::custom_keyword!(property);

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
    syn::custom_keyword!(notify);

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
    Variant(keywords::variant, Option<syn::LitStr>),
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

pub enum PropertyVirtual {
    Virtual(Token![virtual]),
    Delegate(Box<syn::Expr>),
}

pub enum PropertyName {
    Field(syn::Ident),
    Custom(syn::LitStr)
}

pub struct Property {
    pub field: Option<syn::Field>,
    pub public: bool,
    pub skip: bool,
    pub type_: PropertyType,
    pub notify_public: bool,
    pub virtual_: Option<PropertyVirtual>,
    pub override_: Option<syn::Type>,
    pub get: Option<Option<syn::Path>>,
    pub set: Option<Option<syn::Path>>,
    pub name: PropertyName,
    pub nick: Option<syn::LitStr>,
    pub blurb: Option<syn::LitStr>,
    pub minimum: Option<syn::Lit>,
    pub maximum: Option<syn::Lit>,
    pub default: Option<syn::Lit>,
    pub flags: PropertyFlags,
    pub flag_spans: Vec<Span>,
}

impl Property {
    pub fn create(&self, go: &syn::Ident) -> TokenStream {
        let glib = quote! { #go::glib };
        let name = self.name();
        if let Some(iface) = &self.override_ {
            return quote! {
                #glib::ParamSpecOverride::for_interface::<#iface>(#name)
            };
        }
        let nick = self.nick.as_ref().map(|s| s.value()).unwrap_or_else(|| name.clone());
        let blurb = self.blurb.as_ref().map(|s| s.value()).unwrap_or_else(|| name.clone());
        let flags = self
            .flags
            .tokens(&glib, self.get.is_some(), self.set.is_some());
        let ty = &self.field.as_ref().expect("no field").ty;
        let static_type = quote! {
            <<<#ty as #go::ParamStore>::Type as #glib::value::ValueType>::Type as #glib::StaticType>::static_type(),
        };
        match &self.type_ {
            PropertyType::Unspecified => {
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
            PropertyType::Enum(_) => {
                let default = self
                    .default
                    .as_ref()
                    .map(|p| p.to_token_stream())
                    .unwrap_or_else(|| quote! { 0 });
                quote! {
                    #glib::ParamSpecEnum::new(
                        #name, #nick, #blurb,
                        #static_type,
                        #default,
                        #flags
                    )
                }
            }
            PropertyType::Flags(_) => {
                let default = self
                    .default
                    .as_ref()
                    .map(|p| p.to_token_stream())
                    .unwrap_or_else(|| quote! { 0 });
                quote! {
                    #glib::ParamSpecFlags::new(
                        #name, #nick, #blurb,
                        #static_type,
                        #default,
                        #flags
                    )
                }
            }
            PropertyType::Boxed(_) => quote! {
                #glib::ParamSpecBoxed::new(
                    #name, #nick, #blurb,
                    #static_type,
                    #flags
                )
            },
            PropertyType::Object(_) => quote! {
                #glib::ParamSpecObject::new(
                    #name, #nick, #blurb,
                    #static_type,
                    #flags
                )
            },
            PropertyType::Variant(_, element) => {
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
    pub fn name(&self) -> String {
        match &self.name {
            PropertyName::Field(name) => name.to_string().to_kebab_case(),
            PropertyName::Custom(name) => name.value()
        }
    }
    pub fn name_span(&self) -> Span {
        match &self.name {
            PropertyName::Field(name) => name.span(),
            PropertyName::Custom(name) => name.span()
        }
    }
    fn inner_type(&self, go: &syn::Ident) -> TokenStream {
        let ty = &self.field.as_ref().expect("no field for inner type").ty;
        quote! { <#ty as #go::ParamStore>::Type }
    }
    fn field_storage(&self, go: Option<&syn::Ident>) -> TokenStream {
        if let Some(PropertyVirtual::Delegate(delegate)) = &self.virtual_ {
            quote! { #delegate }
        } else {
            let field = self
                .field
                .as_ref()
                .expect("no field")
                .ident
                .as_ref()
                .expect("no field ident");
            let recv = if let Some(go) = go {
                quote! { #go::glib::subclass::prelude::ObjectSubclassIsExt::imp(self) }
            } else {
                quote! { self }
            };
            quote! { #recv.#field }
        }
    }
    fn method_call(
        method_name: &syn::Ident,
        args: TokenStream,
        trait_name: Option<&syn::Ident>,
        glib: &TokenStream
    ) -> TokenStream {
        if let Some(trait_name) = trait_name {
            quote! {
                <<Self as #glib::subclass::types::ObjectSubclass>::Type as #trait_name>::#method_name(obj, #args)
            }
        } else {
            quote! { obj.#method_name(#args) }
        }
    }
    pub fn get_impl(
        &self,
        index: usize,
        trait_name: Option<&syn::Ident>,
        glib: &TokenStream,
    ) -> Option<TokenStream> {
        self.get.as_ref().map(|expr| {
            let expr = expr
                .as_ref()
                .map(|expr| quote! { #expr() })
                .unwrap_or_else(|| {
                    let method_name = format_ident!("{}", self.name().to_snake_case());
                    Self::method_call(&method_name, quote! {}, trait_name, glib)
                });
            quote! {
                #index => {
                    #glib::ToValue::to_value(&#expr)
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
            let field = self.field_storage(Some(&go));
            let ty = self.inner_type(go);
            quote! {
                #proto {
                    #go::ParamStoreRead::get_value(&#field).get::<#ty>().unwrap()
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
        self.set.as_ref().map(|expr| {
            let ty = self.inner_type(go);
            let set = if let Some(expr) = &expr {
                quote! { #expr(value.get::<#ty>().unwrap()); }
            } else if self.flags.contains(PropertyFlags::EXPLICIT_NOTIFY) {
                let method_name = format_ident!("set_{}", self.name().to_snake_case());
                let glib = quote! { #go::glib };
                Self::method_call(&method_name, quote! { value }, trait_name, &glib)
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
        trait_name: &TokenStream,
        go: &syn::Ident,
    ) -> Option<TokenStream> {
        matches!(self.set, Some(None)).then(|| {
            let proto = self.setter_prototype(go).expect("no setter proto");
            let body = if self.flags.contains(PropertyFlags::EXPLICIT_NOTIFY) {
                let field = self.field_storage(Some(&go));
                quote! {
                    if #go::ParamStoreWrite::set(&#field, &value) {
                        self.notify_by_pspec(&<<Self as #go::glib::object::ObjectSubclassIs>::Subclass as #trait_name>::properties()[#index]);
                    }
                }
            } else {
                let name = self.name();
                quote! {
                    self.set_property(#name, value);
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
        trait_name: &TokenStream,
        glib: &TokenStream,
    ) -> TokenStream {
        let proto = self.pspec_prototype(glib);
        quote! {
            #proto {
                &<<Self as #glib::object::ObjectSubclassIs>::Subclass as #trait_name>::properties()[#index]
            }
        }
    }
    pub fn notify_prototype(&self) -> TokenStream {
        let method_name = format_ident!("notify_{}", self.name().to_snake_case());
        quote! { fn #method_name(&self) }
    }
    pub fn notify_definition(
        &self,
        index: usize,
        trait_name: &TokenStream,
        glib: &TokenStream,
    ) -> TokenStream {
        let proto = self.notify_prototype();
        quote! {
            #proto {
                self.notify_by_pspec(&<<Self as #glib::object::ObjectSubclassIs>::Subclass as #trait_name>::properties()[#index]);
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
            type_: PropertyType::Unspecified,
            notify_public: false,
            virtual_: None,
            override_: None,
            get: pod.then(|| None),
            set: pod.then(|| None),
            name: PropertyName::Field(field.ident.clone().expect("no field ident")),
            nick: None,
            blurb: None,
            minimum: None,
            maximum: None,
            default: None,
            flags: PropertyFlags::empty(),
            flag_spans: vec![],
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
        if prop.virtual_.is_none() {
            prop.field = Some(field);
        }
        Ok(prop)
    }
    fn parse_attr(
        stream: syn::parse::ParseStream,
        field: &syn::Field,
        pod: bool,
    ) -> syn::Result<Self> {
        let mut prop = Self::new(field, pod);
        let mut begin = true;
        prop.skip = false;
        if stream.is_empty() {
            return Ok(prop);
        }
        let input;
        syn::parenthesized!(input in stream);
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
                if matches!(prop.type_, PropertyType::Unspecified) {
                    prop.type_ = PropertyType::Enum(kw);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `enum`, `flags`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::flags) {
                let kw = input.parse::<keywords::flags>()?;
                if matches!(prop.type_, PropertyType::Unspecified) {
                    prop.type_ = PropertyType::Flags(kw);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `enum`, `flags`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::boxed) {
                let kw = input.parse::<keywords::boxed>()?;
                if matches!(prop.type_, PropertyType::Unspecified) {
                    prop.type_ = PropertyType::Boxed(kw);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `flags`, `enum`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::object) {
                let kw = input.parse::<keywords::object>()?;
                if matches!(prop.type_, PropertyType::Unspecified) {
                    prop.type_ = PropertyType::Object(kw);
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `flags`, `enum`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(keywords::variant) {
                let kw = input.parse::<keywords::variant>()?;
                if matches!(prop.type_, PropertyType::Unspecified) {
                    if input.peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        let element = input.parse::<syn::LitStr>()?;
                        prop.type_ = PropertyType::Variant(kw, Some(element));
                    } else {
                        prop.type_ = PropertyType::Variant(kw, None);
                    }
                } else {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `enum`, `flags`, `boxed`, `object`, `variant` is allowed",
                    ));
                }
            } else if lookahead.peek(Token![override]) {
                let kw = input.parse::<Token![override]>()?;
                if prop.override_.is_some() {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Duplicate `override` attribute",
                    ));
                }
                input.parse::<Token![=]>()?;
                prop.override_.replace(input.parse()?);
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
            } else if lookahead.peek(keywords::notify) {
                let kw = input.parse::<keywords::notify>()?;
                if prop.notify_public {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Duplicate `notify` attribute",
                    ));
                }
                input.parse::<Token![=]>()?;
                input.parse::<Token![pub]>()?;
                prop.notify_public = true;
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
            begin = false;
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        // validation
        let name = prop.name();
        if !super::is_valid_name(&name) {
            return Err(syn::Error::new(
                prop.name_span(),
                format!("Invalid property name '{}'. Property names must start with an ASCII letter and only contain ASCII letters, numbers, '-' or '_'", name),
            ))
        }
        match &field.vis {
            syn::Visibility::Inherited | syn::Visibility::Public(_) => {}
            vis => {
                return Err(syn::Error::new_spanned(
                    vis,
                    "Only `pub` or private is allowed for property visibility",
                ))
            }
        };
        if let Some(_) = &prop.override_ {
            if let Some(nick) = &prop.nick {
                return Err(syn::Error::new_spanned(nick, "`nick` not allowed on override property"));
            }
            if let Some(blurb) = &prop.blurb {
                return Err(syn::Error::new_spanned(blurb, "`blurb` not allowed on override property"));
            }
            if let Some(minimum) = &prop.minimum {
                return Err(syn::Error::new_spanned(minimum, "`minimum` not allowed on override property"));
            }
            if let Some(maximum) = &prop.maximum {
                return Err(syn::Error::new_spanned(maximum, "`maximum` not allowed on override property"));
            }
            if let Some(default) = &prop.default {
                return Err(syn::Error::new_spanned(default, "`default` not allowed on override property"));
            }
            if let Some(flag) = prop.flag_spans.first() {
                return Err(syn::Error::new(flag.clone(), "flag not allowed on override property"));
            }
            if let Some(span) = prop.type_.span() {
                return Err(syn::Error::new(span.clone(), "type specifier not allowed on override property"));
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

        Ok(prop)
    }
}
