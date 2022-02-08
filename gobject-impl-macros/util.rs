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
pub fn has_method(items: &[syn::ImplItem], method: &str) -> (bool, syn::Ident) {
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

pub enum ClassInheritance {
    Final,
    Abstract(syn::Ident),
}

pub struct Args {
    pub type_: Option<syn::Type>,
    pub inheritance: ClassInheritance,
    pub pod: bool,
}

impl Args {
    pub fn parse(input: syn::parse::ParseStream, interface: bool) -> syn::Result<Self> {
        let mut type_ = None;
        let mut inheritance = None;
        let mut pod = false;

        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if lookahead.peek(Token![type]) {
                let kw = input.parse::<Token![type]>()?;
                if type_.is_some() {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `type` attribute"));
                }
                input.parse::<Token![=]>()?;
                type_.replace(input.parse()?);
            } else if lookahead.peek(Token![trait]) {
                let kw = input.parse::<Token![trait]>()?;
                if inheritance.is_some() {
                    if interface {
                        return Err(syn::Error::new_spanned(kw, "Duplicate `trait` attribute"));
                    } else {
                        return Err(syn::Error::new_spanned(
                            kw,
                            "Only one of `trait`, `final` is allowed",
                        ));
                    }
                }
                input.parse::<Token![=]>()?;
                inheritance.replace(ClassInheritance::Abstract(input.parse()?));
            } else if lookahead.peek(keywords::pod) {
                let kw = input.parse::<keywords::pod>()?;
                if pod {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `pod` attribute"));
                }
                pod = true;
            } else if !interface && lookahead.peek(Token![final]) {
                let kw = input.parse::<Token![final]>()?;
                if inheritance.is_some() {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Only one of `trait`, `final` is allowed",
                    ));
                }
                inheritance.replace(ClassInheritance::Final);
            } else {
                return Err(lookahead.error());
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        if type_.is_none() {
            if interface {
                return Err(syn::Error::new(input.span(), "`type` attribute required"));
            }
            if matches!(inheritance, Some(ClassInheritance::Final)) {
                return Err(syn::Error::new(
                    input.span(),
                    "`type` attribute required on final class",
                ));
            }
        }
        let inheritance = inheritance.ok_or_else(|| {
            let msg = if interface {
                "`trait` attribute required"
            } else {
                "`trait` or `final` attribute required"
            };
            syn::Error::new(input.span(), msg)
        })?;
        Ok(Args {
            type_,
            inheritance,
            pod,
        })
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
        let signals = Signal::from_items(&mut item.items, is_interface)?;

        let mut properties = vec![];
        let mut struct_item = None;
        let mut index = 0;
        loop {
            if index >= item.items.len() {
                break;
            }
            let mut struct_def = false;
            if let syn::ImplItem::Macro(mac) = &item.items[index] {
                if mac.mac.path.is_ident("properties") {
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
            if struct_def {
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
    pub public_methods: TokenStream,
}

impl Output {
    pub fn new(
        definition: &mut ObjectDefinition,
        object_type: Option<&syn::Type>,
        inheritance: &ClassInheritance,
        signals_path: &TokenStream,
        properties_path: &TokenStream,
        go: &syn::Ident,
    ) -> Self {
        let ObjectDefinition {
            item,
            signals,
            properties,
            ..
        } = definition;
        let glib = quote! { #go::glib };

        let mut private_impl_methods = vec![];
        let mut prototypes = vec![];
        let mut methods = vec![];

        if !signals.is_empty() {
            let signals = signals
                .iter()
                .map(|signal| signal.create(&item.self_ty, object_type, &glib));
            let signal_defs = quote! { vec![ #(#signals),* ] };
            let static_def = quote! {
                static SIGNALS: #glib::once_cell::sync::Lazy<::std::vec::Vec<#glib::subclass::Signal>> = #glib::once_cell::sync::Lazy::new(|| {
                    #signal_defs
                });
                ::core::convert::AsRef::as_ref(::std::ops::Deref::deref(&SIGNALS))
            };
            let (has_signals, signals_ident) = has_method(&item.items, "signals");
            if has_signals {
                private_impl_methods.push(quote! {
                    fn #signals_ident() -> Vec<#glib::subclass::Signal> {
                        #signal_defs
                    }
                });
            } else {
                item.items.push(syn::ImplItem::Verbatim(quote! {
                    fn #signals_ident() -> &'static [#glib::subclass::Signal] {
                        #static_def
                    }
                }));
            }
        }

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
            if let Some(set) = prop.set_impl(index, go) {
                prop_set_impls.push(set);
            }
            if let Some(get) = prop.get_impl(index, go) {
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
            if let Some(pspec) = prop.pspec_prototype(&glib) {
                prototypes.push(make_stmt(pspec));
                methods.push(
                    prop.pspec_definition(index, properties_path, &glib)
                        .expect("no pspec definition"),
                );
            }
            if let Some(notify) = prop.notify_prototype() {
                prototypes.push(make_stmt(notify));
                methods.push(
                    prop.notify_definition(index, properties_path, &glib)
                        .expect("no notify definition"),
                );
            }
            if let Some(connect_notify) = prop.connect_prototype(&glib) {
                prototypes.push(make_stmt(connect_notify));
                methods.push(
                    prop.connect_definition(&glib)
                        .expect("no connect notify definition"),
                );
            }
            if let Some(getter) = prop.getter_prototype(go) {
                prototypes.push(make_stmt(getter));
                methods.push(
                    prop.getter_definition(&self_ty, go)
                        .expect("no getter definition"),
                );
            }
            if let Some(borrow) = prop.borrow_prototype(go) {
                prototypes.push(make_stmt(borrow));
                methods.push(
                    prop.borrow_definition(&self_ty, go)
                        .expect("no borrow definition"),
                );
            }
            if let Some(setter) = prop.setter_prototype(go) {
                prototypes.push(make_stmt(setter));
                methods.push(
                    prop.setter_definition(index, &self_ty, properties_path, go)
                        .expect("no setter definition"),
                );
            }
        }

        let public_methods = match inheritance {
            ClassInheritance::Final => {
                let object_type = object_type.expect("no object_type");
                let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
                quote! {
                    impl #impl_generics #object_type #ty_generics #where_clause {
                        #(pub #methods)*
                    }
                }
            }
            ClassInheritance::Abstract(trait_name) => {
                let type_var = format_ident!("____Object");
                let mut generics = item.generics.clone();
                {
                    let param = quote! { #type_var };
                    generics.params.push(syn::parse2(param).unwrap());
                    let where_clause = generics.make_where_clause();
                    let predicate = quote! { #type_var: #glib::IsA<#self_ty> };
                    where_clause
                        .predicates
                        .push(syn::parse2(predicate).unwrap());
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
            }
        };

        Self {
            private_impl_methods,
            prop_set_impls,
            prop_get_impls,
            prop_defs,
            public_methods,
        }
    }
}
