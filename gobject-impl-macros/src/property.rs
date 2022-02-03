use heck::{ToKebabCase, ToSnakeCase};
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, format_ident, ToTokens};
use syn::Token;

mod keywords {
    // property keywords
    syn::custom_keyword!(property);

    // prop attributes
    syn::custom_keyword!(skip);
    syn::custom_keyword!(get);
    syn::custom_keyword!(set);
    syn::custom_keyword!(nick);
    syn::custom_keyword!(blurb);
    syn::custom_keyword!(minimum);
    syn::custom_keyword!(maximum);
    syn::custom_keyword!(default);
    syn::custom_keyword!(flags);
    syn::custom_keyword!(boxed);
    syn::custom_keyword!(object);
    syn::custom_keyword!(nullable);
    syn::custom_keyword!(delegate);
    syn::custom_keyword!(construct);
    syn::custom_keyword!(construct_only);
    syn::custom_keyword!(lax_validation);
    syn::custom_keyword!(explicit_notify);
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
    fn to_tokens(&self, glib: &TokenStream2) -> TokenStream2 {
        let count = Self::empty().bits().leading_zeros() - Self::all().bits().leading_zeros();
        let mut flags = vec![];
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
    Variant(Option<String>)
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
            _ => return None
        };
        let seg = path.segments.last()?;
        let bracketed = match &seg.arguments {
            syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { args, .. }) => args,
            _ => return None
        };
        let arg = bracketed.last()?;
        let ty = match arg {
            syn::GenericArgument::Type(ty) => ty,
            _ => return None
        };
        Some(ty)
    }
}

pub enum PropertyVirtual {
    Virtual,
    Delegate(syn::Expr),
}

pub struct Property {
    pub field: Option<syn::Field>,
    pub type_: PropertyType,
    pub public: bool,
    pub skip: bool,
    pub virtual_: Option<PropertyVirtual>,
    pub get: Option<Option<syn::Expr>>,
    pub set: Option<Option<syn::Expr>>,
    pub name: Option<String>,
    pub nick: Option<String>,
    pub blurb: Option<String>,
    pub minimum: Option<syn::Lit>,
    pub maximum: Option<syn::Lit>,
    pub default: Option<syn::Lit>,
    pub flags: PropertyFlags,
    pub nullable: Option<keywords::nullable>,
}

impl Property {
    pub fn create(&self, go: &syn::Ident) -> TokenStream2 {
        let glib = quote! { #go::glib };
        let name = self.name.as_ref().unwrap();
        let nick = self.nick.as_ref().unwrap();
        let blurb = self.blurb.as_ref().unwrap();
        let flags = self.flags.to_tokens(&glib);
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
            },
            PropertyType::Enum(ty) => {
                let default = self.default
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
            },
            PropertyType::Flags(ty) => {
                let default = self.default
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
            },
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
            },
        }
    }
    fn name_ident(&self) -> String {
        self.name.as_ref().unwrap().to_snake_case()
    }
    pub fn get_impl(&self) -> Option<TokenStream2> {
        todo!()
    }
    pub fn getter_prototype(&self) -> Option<TokenStream2> {
        self.get.as_ref().map(|_| {
            let method_name = format_ident!("{}", self.name_ident());
            let ty = self.type_.inner_type().unwrap();
            quote! { fn #method_name(&self) -> #ty }
        })
    }
    pub fn getter_definition(&self) -> Option<TokenStream2> {
        todo!()
    }
    pub fn set_impl(&self) -> Option<TokenStream2> {
        todo!()
    }
    pub fn setter_prototype(&self) -> Option<TokenStream2> {
        self.get.as_ref().map(|_| {
            let method_name = format_ident!("set_{}", self.name_ident());
            let ty = self.type_.inner_type().unwrap();
            quote! { fn #method_name(&self, value: #ty) }
        })
    }
    pub fn setter_definition(&self) -> Option<TokenStream2> {
        todo!()
    }
    pub fn pspec_prototype(&self, glib: &TokenStream2) -> TokenStream2 {
        let method_name = format_ident!("pspec_{}", self.name_ident());
        quote! { fn #method_name(&self) -> &'static #glib::ParamSpec }
    }
    pub fn pspec_definition(&self, index: usize, trait_name: &TokenStream2, glib: &TokenStream2) -> TokenStream2 {
        let proto = self.pspec_prototype(glib);
        quote! {
            #proto {
                &<Self as #trait_name>::properties()[#index]
            }
        }
    }
    pub fn notify_prototype(&self) -> TokenStream2 {
        let method_name = format_ident!("notify_{}", self.name_ident());
        quote! { fn #method_name(&self) }
    }
    pub fn notify_definition(&self, index: usize, trait_name: &TokenStream2) -> TokenStream2 {
        let proto = self.notify_prototype();
        quote! {
            #proto {
                self.notify_by_pspec(&<Self as #trait_name>::properties()[#index]);
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
            type_: PropertyType::Unspecified(field.ty.clone()),
            public: !matches!(&field.vis, syn::Visibility::Inherited),
            skip: !pod,
            virtual_: None,
            get: pod.then(|| None),
            set: pod.then(|| None),
            name: None,
            nick: None,
            blurb: None,
            minimum: None,
            maximum: None,
            default: None,
            flags: PropertyFlags::empty(),
            nullable: None,
        }
    }
    pub fn parse(mut field: syn::Field, pod: bool) -> syn::Result<Self> {
        let attr_pos = field.attrs.iter().position(|f| f.path.is_ident("property"));
        let mut prop = if let Some(pos) = attr_pos {
            let attr = field.attrs.remove(pos);
            syn::parse::Parser::parse2(
                super::constrain(|item| Self::parse_attr(item, &field, pod)),
                attr.tokens
            )?
        } else {
            Self::new(&field, pod)
        };
        if !prop.skip {
            let name = prop.name.get_or_insert_with(|| {
                field.ident.as_ref().unwrap().to_string().to_kebab_case()
            });
            prop.nick.get_or_insert_with(|| name.clone());
            prop.blurb.get_or_insert_with(|| name.clone());
            if let Some(nullable) = &prop.nullable {
                if !matches!(
                    &prop.type_,
                    PropertyType::Object(_) |
                    PropertyType::Boxed(_) |
                    PropertyType::Variant(_)
                ) {
                    return Err(syn::Error::new_spanned(nullable, "`nullable` only allowed on properties of type `object`, `boxed`, or `variant`"));
                }
            }
        }
        if prop.virtual_.is_none() {
            prop.field = Some(field);
        }
        Ok(prop)
    }
    fn parse_attr(input: syn::parse::ParseStream, field: &syn::Field, pod: bool) -> syn::Result<Self> {
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
            }
            begin = false;
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(prop)
    }
}
