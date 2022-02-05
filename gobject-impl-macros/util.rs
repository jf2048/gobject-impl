use std::collections::HashSet;

use heck::ToKebabCase;
use heck::ToSnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Token;

use super::property::*;
use super::signal::*;

pub fn go_crate_ident() -> syn::Ident {
    use proc_macro_crate::FoundCrate;

    let crate_name = match proc_macro_crate::crate_name("gobject-impl") {
        Ok(FoundCrate::Name(name)) => name,
        _ => "gobject_impl".to_owned(),
    };

    syn::Ident::new(&crate_name, proc_macro2::Span::call_site())
}

pub fn is_valid_name(name: &str) -> bool {
    let mut iter = name.chars();
    if let Some(c) = iter.next() {
        if !c.is_ascii_alphabetic() {
            return false;
        }
        for c in iter {
            if !c.is_ascii_alphanumeric() && c != '-' && c != '_' {
                return false;
            }
        }
        true
    } else {
        false
    }
}

#[inline]
pub fn has_method(items: &Vec<syn::ImplItem>, method: &str) -> (bool, syn::Ident) {
    let res = items.iter().any(|i| match i {
        syn::ImplItem::Method(m) => m.sig.ident == method,
        _ => false,
    });
    (
        res,
        if res {
            format_ident!("inner_{}", method)
        } else {
            format_ident!("{}", method)
        },
    )
}

#[inline]
pub fn constrain<F, T>(f: F) -> F
where
    F: for<'r> Fn(&'r syn::parse::ParseBuffer<'r>) -> syn::Result<T>,
{
    f
}

#[inline]
pub fn make_stmt(tokens: TokenStream) -> TokenStream {
    quote! { #tokens; }
}

mod keywords {
    syn::custom_keyword!(pod);
}

pub struct Args {
    pub type_: Option<syn::Type>,
    pub trait_: syn::Ident,
    pub pod: bool,
}

impl Args {
    pub fn parse(input: syn::parse::ParseStream, interface: bool) -> syn::Result<Self> {
        let mut type_ = None;
        let mut trait_ = None;
        let mut pod = false;

        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if interface && lookahead.peek(Token![type]) {
                let kw = input.parse::<Token![type]>()?;
                if type_.is_some() {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `type` attribute"));
                }
                input.parse::<Token![=]>()?;
                type_.replace(input.parse()?);
            } else if lookahead.peek(Token![trait]) {
                let kw = input.parse::<Token![trait]>()?;
                if trait_.is_some() {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `trait` attribute"));
                }
                input.parse::<Token![=]>()?;
                trait_.replace(input.parse()?);
            } else if lookahead.peek(keywords::pod) {
                let kw = input.parse::<keywords::pod>()?;
                if pod {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `pod` attribute"));
                }
                pod = true;
            } else {
                return Err(lookahead.error());
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        let trait_ =
            trait_.ok_or_else(|| syn::Error::new(input.span(), "`trait` attribute required"))?;
        Ok(Args { type_, trait_, pod })
    }
}

pub struct ObjectDefinition {
    pub item: syn::ItemImpl,
    pub struct_item: Option<syn::ItemStruct>,
    pub properties: Vec<Property>,
    pub signals: Vec<Signal>,
}

impl ObjectDefinition {
    pub fn new(mut item: syn::ItemImpl, pod: bool, is_interface: bool) -> syn::Result<Self> {
        let mut properties = vec![];
        let mut signal_names = HashSet::new();
        let mut signals = Vec::<Signal>::new();

        let mut struct_item = None;
        let mut index = 0;
        loop {
            if index >= item.items.len() {
                break;
            }
            let mut signal_attr = None;
            let mut struct_def = false;
            {
                let sub = &mut item.items[index];
                match sub {
                    syn::ImplItem::Method(method) => {
                        let signal_index = method.attrs.iter().position(|attr| {
                            attr.path.is_ident("signal") || attr.path.is_ident("accumulator")
                        });
                        if let Some(signal_index) = signal_index {
                            signal_attr.replace(method.attrs.remove(signal_index));
                        }
                        if let Some(next) = method.attrs.first() {
                            return Err(syn::Error::new_spanned(
                                next,
                                "Unknown attribute on signal",
                            ));
                        }
                    }
                    syn::ImplItem::Macro(mac) => {
                        let p = &mac.mac.path;
                        if p.is_ident("properties") {
                            if struct_item.is_some() {
                                return Err(syn::Error::new_spanned(
                                    mac,
                                    "Duplicate `properties` definition in trait impl",
                                ));
                            }
                            if !matches!(mac.mac.delimiter, syn::MacroDelimiter::Brace(_)) {
                                return Err(syn::Error::new_spanned(
                                    &mac.mac,
                                    "`properties` macro must have braces",
                                ));
                            }
                            struct_def = true;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(attr) = signal_attr {
                let sub = item.items.remove(index);
                let mut method = match sub {
                    syn::ImplItem::Method(method) => method,
                    _ => unreachable!(),
                };
                if attr.path.is_ident("signal") {
                    let signal = {
                        let ident = &method.sig.ident;
                        let signal_attrs = syn::parse2::<SignalAttrs>(attr.tokens.clone())?;
                        let name = signal_attrs
                            .name
                            .clone()
                            .unwrap_or_else(|| ident.to_string().to_kebab_case());
                        if !is_valid_name(&name) {
                            if let Some(name) = &signal_attrs.name {
                                return Err(syn::Error::new_spanned(name, format!("Invalid signal name '{}'. Signal names must start with an ASCII letter and only contain ASCII letters, numbers, '-' or '_'", name)));
                            } else {
                                return Err(syn::Error::new_spanned(&ident, format!("Invalid signal name '{}'. Signal names must start with an ASCII letter and only contain ASCII letters, numbers, '-' or '_'", ident)));
                            }
                        }
                        if signal_names.contains(&name) {
                            return Err(syn::Error::new_spanned(
                                ident,
                                format!("Duplicate definition for signal `{}`", name),
                            ));
                        }
                        let signal = if let Some(i) = signals.iter().position(|s| s.ident == *ident)
                        {
                            &mut signals[i]
                        } else {
                            signals.push(Signal::new(ident.clone()));
                            signals.last_mut().unwrap()
                        };
                        if signal.handler.is_some() {
                            return Err(syn::Error::new_spanned(
                                &ident,
                                format!("Duplicate definition for signal `{}`", ident),
                            ));
                        }
                        signal_names.insert(name.clone());
                        signal.name = name;
                        signal.flags = signal_attrs.flags;
                        signal.emit = signal_attrs.emit;
                        signal.connect = signal_attrs.connect;
                        signal.interface = is_interface;
                        signal
                    };
                    method.sig.ident =
                        format_ident!("{}_class_handler", &signal.name.to_snake_case());
                    signal.handler = Some(method);
                } else if attr.path.is_ident("accumulator") {
                    if !attr.tokens.is_empty() {
                        return Err(syn::Error::new_spanned(
                            &attr.tokens,
                            "Unknown token on accumulator",
                        ));
                    }
                    if matches!(method.sig.output, syn::ReturnType::Default) {
                        return Err(syn::Error::new_spanned(
                            method.sig.output,
                            "accumulator must have return type",
                        ));
                    }
                    let signal = {
                        let ident = &method.sig.ident;
                        let signal = if let Some(i) = signals.iter().position(|s| s.ident == *ident)
                        {
                            &mut signals[i]
                        } else {
                            signals.push(Signal::new(ident.clone()));
                            signals.last_mut().unwrap()
                        };
                        if signal.accumulator.is_some() {
                            return Err(syn::Error::new_spanned(&ident, format!("Duplicate definition for accumulator on signal definition `{}`", ident)));
                        }
                        signal
                    };
                    method.sig.ident = format_ident!("____accumulator");
                    signal.accumulator = Some(method);
                } else {
                    unreachable!();
                }
            } else if struct_def {
                let sub = item.items.remove(index);
                let mac = match sub {
                    syn::ImplItem::Macro(mac) => mac,
                    _ => unreachable!(),
                };
                let mut si = syn::parse2::<syn::ItemStruct>(mac.mac.tokens)?;
                properties = Property::from_struct(&mut si, pod, is_interface)?;
                struct_item.replace(si);
            } else {
                index += 1;
            }
        }

        if is_interface {
            if let Some(p) = properties.iter().find(|p| p.skip) {
                return Err(syn::Error::new_spanned(
                    &p.ty,
                    "Interface field must be a property",
                ));
            }
        }

        Ok(ObjectDefinition {
            item,
            struct_item,
            properties,
            signals,
        })
    }
}

pub struct Output {
    pub private_impl_methods: Vec<TokenStream>,
    pub prop_set_impls: Vec<TokenStream>,
    pub prop_get_impls: Vec<TokenStream>,
    pub prop_defs: Option<TokenStream>,
    pub signal_defs: Option<TokenStream>,
    pub ext_trait: Option<TokenStream>,
}

impl Output {
    pub fn new(
        item: &syn::ItemImpl,
        signals: &[Signal],
        properties: &[Property],
        object_type: Option<&syn::Type>,
        trait_name: Option<&syn::Ident>,
        signals_path: &TokenStream,
        properties_path: &TokenStream,
        go: &syn::Ident,
    ) -> Self {
        let glib = quote! { #go::glib };

        let mut private_impl_methods = vec![];
        let mut prototypes = vec![];
        let mut methods = vec![];

        let signal_defs = if signals.is_empty() {
            None
        } else {
            let signals = signals
                .iter()
                .map(|signal| signal.create(&item.self_ty, object_type, &glib));
            Some(quote! {
                static SIGNALS: #glib::once_cell::sync::Lazy<::std::vec::Vec<#glib::subclass::Signal>> = #glib::once_cell::sync::Lazy::new(|| {
                    vec![
                        #(#signals),*
                    ]
                });
                ::core::convert::AsRef::as_ref(::std::ops::Deref::deref(&SIGNALS))
            })
        };

        for (index, signal) in signals.iter().enumerate() {
            prototypes.push(make_stmt(signal.signal_prototype(&glib)));
            methods.push(signal.signal_definition(index, signals_path, &glib));
            if signal.emit {
                prototypes.push(make_stmt(signal.emit_prototype(&glib)));
                methods.push(signal.emit_definition(index, signals_path, &glib));
            }
            if signal.connect {
                prototypes.push(make_stmt(signal.connect_prototype(&glib)));
                methods.push(signal.connect_definition(index, signals_path, &glib));
            }

            if let Some(method) = signal.handler_definition() {
                private_impl_methods.push(method);
            }
        }

        let mut props = vec![];
        let mut prop_set_impls = vec![];
        let mut prop_get_impls = vec![];
        for (index, prop) in properties.iter().filter(|p| !p.skip).enumerate() {
            props.push(prop.create(go));
            let index = index + 1;
            if let Some(set) = prop.set_impl(index, trait_name, go) {
                prop_set_impls.push(set);
            }
            if let Some(get) = prop.get_impl(index, trait_name, go) {
                prop_get_impls.push(get);
            }
        }

        let prop_defs = if props.is_empty() {
            None
        } else {
            Some(quote! {
                static PROPS: #glib::once_cell::sync::Lazy<::std::vec::Vec<#glib::ParamSpec>> = #glib::once_cell::sync::Lazy::new(|| {
                    vec![#(#props),*]
                });
                ::core::convert::AsRef::as_ref(::std::ops::Deref::deref(&PROPS))
            })
        };

        let self_ty = if let Some(object_type) = object_type {
            quote! { #object_type }
        } else {
            let self_ty = &item.self_ty;
            quote! { <#self_ty as #glib::subclass::types::ObjectSubclass>::Type }
        };
        for (index, prop) in properties.iter().enumerate() {
            if prop.skip {
                continue;
            }
            prototypes.push(make_stmt(prop.pspec_prototype(&glib)));
            methods.push(prop.pspec_definition(index, properties_path, &glib));
            if prop.notify {
                prototypes.push(make_stmt(prop.notify_prototype()));
                methods.push(prop.notify_definition(index, properties_path, &glib));
            }
            if prop.connect_notify {
                prototypes.push(make_stmt(prop.connect_prototype(&glib)));
                methods.push(prop.connect_definition(&glib));
            }
            if let Some(getter) = prop.getter_prototype(go) {
                prototypes.push(make_stmt(getter));
                methods.push(prop.getter_definition(&self_ty, go).expect("no getter definition"));
            }
            if let Some(setter) = prop.setter_prototype(go) {
                prototypes.push(make_stmt(setter));
                methods.push(
                    prop.setter_definition(index, &self_ty, properties_path, go)
                        .expect("no setter definition"),
                );
            }
        }

        let ext_trait = trait_name.as_ref().map(|trait_name| {
            let type_var = format_ident!("____Object");
            let mut generics = item.generics.clone();
            {
                let param = quote! { #type_var };
                generics.params.push(syn::parse2(param).unwrap());
                let where_clause = generics.make_where_clause();
                let predicate = quote! { #type_var: #glib::IsA<#self_ty> };
                where_clause.predicates.push(syn::parse2(predicate).unwrap());
            }
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let (_, ty_generics, _) = item.generics.split_for_impl();
            quote! {
                pub trait #trait_name: 'static {
                    #(#prototypes)*
                }
                impl #impl_generics #trait_name for #type_var #ty_generics #where_clause {
                    #(#methods)*
                }
            }
        });

        Self {
            private_impl_methods,
            prop_set_impls,
            prop_get_impls,
            prop_defs,
            signal_defs,
            ext_trait,
        }
    }
}
