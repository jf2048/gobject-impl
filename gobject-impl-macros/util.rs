use heck::ToKebabCase;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use std::collections::{HashMap, HashSet};
use syn::{parse::Parse, Token};

use super::property::*;
use super::signal::{self, *};

pub fn go_crate_ident() -> syn::Ident {
    use proc_macro_crate::FoundCrate;

    let crate_name = match proc_macro_crate::crate_name("gobject-impl") {
        Ok(FoundCrate::Name(name)) => name,
        _ => "gobject-impl".to_owned(),
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
    syn::custom_keyword!(impl_trait);
    syn::custom_keyword!(public_trait);
    syn::custom_keyword!(private_trait);
    syn::custom_keyword!(pod);
}

pub struct Args {
    pub type_: Option<syn::Type>,
    pub impl_trait: Option<Option<syn::Ident>>,
    pub public_trait: Option<syn::Ident>,
    pub private_trait: Option<syn::Ident>,
    pub pod: bool,
}

impl Parse for Args {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut args = Self {
            type_: None,
            impl_trait: None,
            public_trait: None,
            private_trait: None,
            pod: false,
        };
        #[inline]
        fn parse_trait<T: Parse + syn::token::CustomToken + ToTokens>(
            input: &syn::parse::ParseStream,
            storage: &mut Option<syn::Ident>,
        ) -> syn::Result<()> {
            let kw = input.parse::<T>()?;
            if storage.is_some() {
                let msg = format!("Duplicate `{}` attribute", T::display());
                return Err(syn::Error::new_spanned(kw, msg));
            }
            input.parse::<Token![=]>()?;
            storage.replace(input.parse()?);
            Ok(())
        }
        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if lookahead.peek(Token![type]) {
                let kw = input.parse::<Token![type]>()?;
                if args.type_.is_some() {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Duplicate `type` attribute",
                    ));
                }
                input.parse::<Token![=]>()?;
                args.type_.replace(input.parse()?);
            } else if lookahead.peek(keywords::impl_trait) {
                let kw = input.parse::<keywords::impl_trait>()?;
                if args.impl_trait.is_some() {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "Duplicate `impl_trait` attribute",
                    ));
                }
                if input.peek(Token![=]) {
                    input.parse::<Token![=]>()?;
                    args.impl_trait.replace(Some(input.parse()?));
                } else {
                    args.impl_trait.replace(None);
                }
            } else if lookahead.peek(keywords::public_trait) {
                parse_trait::<keywords::public_trait>(&input, &mut args.public_trait)?;
            } else if lookahead.peek(keywords::private_trait) {
                parse_trait::<keywords::private_trait>(&input, &mut args.private_trait)?;
            } else if lookahead.peek(keywords::pod) {
                let kw = input.parse::<keywords::pod>()?;
                if args.pod {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `pod` attribute"));
                }
                args.pod = true;
            } else {
                return Err(lookahead.error());
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        if args.type_.is_some() {
            if let Some(public_trait) = &args.public_trait {
                return Err(syn::Error::new_spanned(public_trait, "`public_trait` not allowed with `type`"));
            }
            if let Some(private_trait) = &args.private_trait {
                return Err(syn::Error::new_spanned(private_trait, "`private_trait` not allowed with `type`"));
            }
        }
        Ok(args)
    }
}

pub enum DefinitionType {
    Object {
        ident: syn::Ident,
    },
    Interface {
        defaultness: Option<Token![default]>,
        unsafety: Option<Token![unsafe]>,
        trait_: Option<(Option<Token![!]>, syn::Path)>,
        self_ty: Box<syn::Type>,
    }
}

pub struct ObjectDefinition {
    pub attrs: Vec<syn::Attribute>,
    pub vis: syn::Visibility,
    pub definition: DefinitionType,
    pub generics: syn::Generics,
    pub properties: Vec<Property>,
    pub signals: Vec<Signal>,
    pub methods: Vec<syn::ImplItemMethod>,
    pub types: Vec<syn::ImplItemType>,
    pub consts: Vec<syn::ImplItemConst>,
}

fn parse_args(content: &syn::parse::ParseBuffer) -> syn::Result<Vec<syn::FnArg>> {
    let args;
    let mut inputs = vec![];
    syn::parenthesized!(args in content);
    while !args.is_empty() {
        inputs.push(args.parse()?);
        if args.is_empty() {
            break;
        }
        args.parse::<Token![,]>()?;
    }
    Ok(inputs)
}

impl ObjectDefinition {
    pub fn header_tokens(&self) -> TokenStream {
        let Self { attrs, vis, generics, .. } = &self;
        match &self.definition {
            DefinitionType::Object {
                ident,
            } => quote! {
                #(#attrs)*
                #vis struct #ident #generics
            },
            DefinitionType::Interface {
                defaultness,
                unsafety,
                trait_,
                self_ty,
            } => {
                let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
                let trait_ = trait_.as_ref().map(|(bang, path)| quote! { #bang #path });
                quote! {
                    #(#attrs)*
                    #vis #defaultness #unsafety impl #impl_generics #trait_ for #self_ty #ty_generics #where_clause
                }
            }
        }
    }
    pub fn parse(input: syn::parse::ParseStream, pod: bool, iface: bool) -> syn::Result<Self> {
        let mut attrs = input.call(syn::Attribute::parse_outer)?;
        let vis = input.parse::<syn::Visibility>()?;

        let (definition, generics) = if iface {
            let defaultness = input.parse()?;
            let unsafety = input.parse()?;
            input.parse::<Token![impl]>()?;

            let has_generics = input.peek(Token![<])
                && (input.peek2(Token![>])
                    || input.peek2(Token![#])
                    || (input.peek2(syn::Ident) || input.peek2(syn::Lifetime))
                        && (input.peek3(Token![:])
                            || input.peek3(Token![,])
                            || input.peek3(Token![>])
                            || input.peek3(Token![=]))
                    || input.peek2(Token![const]));
            let mut generics = if has_generics {
                input.parse()?
            } else {
                syn::Generics::default()
            };

            let polarity = if input.peek(Token![!]) && !input.peek2(syn::token::Brace) {
                Some(input.parse::<Token![!]>()?)
            } else {
                None
            };

            let mut first_ty = input.parse::<syn::Type>()?;

            input.parse::<Token![for]>()?;
            while let syn::Type::Group(ty) = first_ty {
                first_ty = *ty.elem;
            }
            let trait_ = if let syn::Type::Path(syn::TypePath { qself: None, path }) = first_ty {
                Some((polarity, path))
            } else {
                return Err(syn::Error::new_spanned(first_ty, "expected trait path"));
            };
            let self_ty = Box::new(input.parse()?);

            generics.where_clause = input.parse()?;

            (DefinitionType::Interface {
                defaultness,
                unsafety,
                trait_,
                self_ty,
            }, generics)
        } else {
            input.parse::<Token![struct]>()?;
            let ident = input.parse::<syn::Ident>()?;
            let mut generics = input.parse::<syn::Generics>()?;
            if input.peek(Token![where]) {
                generics.where_clause = Some(input.parse()?);
            }
            (DefinitionType::Object { ident }, generics)
        };

        let content;
        syn::braced!(content in input);
        attrs.append(&mut input.call(syn::Attribute::parse_inner)?);

        let mut properties = HashMap::<String, Property>::new();
        let mut signal_names = HashSet::<String>::new();
        let mut signals = HashMap::<syn::Ident, Signal>::new();
        let mut methods = vec![];
        let mut types = vec![];
        let mut consts = vec![];

        loop {
            if content.is_empty() {
                break;
            }
            let attrs = content.call(syn::Attribute::parse_outer)?;
            let vis = content.parse()?;
            if content.peek(signal::keywords::signal) && content.peek2(syn::Ident) {
                content.parse::<signal::keywords::signal>()?;
                let ident: syn::Ident = content.call(syn::ext::IdentExt::parse_any)?;
                let mut signal_attrs = None;
                for attr in attrs {
                    if signal_attrs.is_some() {
                        return Err(syn::Error::new_spanned(
                            attr,
                            "Only one attribute allowed on signal",
                        ));
                    }
                    signal_attrs = Some(syn::parse2::<SignalAttrs>(attr.tokens)?);
                }
                let name = signal_attrs
                    .as_ref()
                    .and_then(|a| a.name.to_owned())
                    .unwrap_or_else(|| ident.to_string().to_kebab_case());
                let inputs = parse_args(&content)?;
                let output = content.parse()?;
                let mut block = None;
                if !content.peek(Token![;]) {
                    block = Some(Box::new(input.parse::<syn::Block>()?));
                } else {
                    content.parse::<Token![;]>()?;
                }

                if signal_names.contains(&name) {
                    return Err(syn::Error::new_spanned(
                        ident,
                        format!("Duplicate definition for signal `{}`", name),
                    ));
                }
                signal_names.insert(name.clone());
                let signal = signals.entry(ident.clone()).or_default();
                if let Some(attrs) = &signal_attrs {
                    signal.flags = attrs.flags;
                    signal.emit_public = attrs.emit_public;
                }
                signal.name = name;
                signal.public = match vis {
                    syn::Visibility::Inherited => false,
                    syn::Visibility::Public(_) => true,
                    vis => {
                        return Err(syn::Error::new_spanned(
                            vis,
                            "Only `pub` or private is allowed for signal visibility",
                        ))
                    }
                };
                signal.inputs = Some(inputs);
                signal.output = output;
                signal.block = block;

                if !is_valid_name(&signal.name) {
                    if let Some(name) = signal_attrs.as_ref().and_then(|s| s.name.as_ref()) {
                        return Err(syn::Error::new_spanned(
                            &name,
                            format!("Invalid signal name '{}'. Signal names must start with an ASCII letter and only contain ASCII letters, numbers, '-' or '_'", name),
                        ));
                    } else {
                        return Err(syn::Error::new_spanned(
                            &ident,
                            format!("Invalid signal name '{}'. Signal names must start with an ASCII letter and only contain ASCII letters, numbers, '-' or '_'", ident),
                        ));
                    }
                }
            } else if content.peek(signal::keywords::signal_accumulator)
                && content.peek2(syn::Ident)
            {
                let kw = content.parse::<signal::keywords::signal_accumulator>()?;
                if !matches!(vis, syn::Visibility::Inherited) {
                    return Err(syn::Error::new_spanned(
                        vis,
                        "signal_accumulator cannot have visibility",
                    ));
                }
                let ident: syn::Ident = content.call(syn::ext::IdentExt::parse_any)?;
                let inputs = parse_args(&content)?;
                let output = content.parse::<syn::ReturnType>()?;
                if matches!(output, syn::ReturnType::Default) {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "signal_accumulator must have return type",
                    ));
                }
                let block = Box::new(input.parse::<syn::Block>()?);

                let signal = signals.entry(ident.clone()).or_default();
                if signal.accumulator.is_some() {
                    return Err(syn::Error::new_spanned(
                        &ident,
                        format!(
                            "Duplicate definition for signal_accumulator on signal definition `{}`",
                            ident
                        ),
                    ));
                }
                signal.accumulator = Some((kw, inputs, block));
            } else if content.peek(Token![fn]) {
                let mut m = content.parse::<syn::ImplItemMethod>()?;
                m.attrs = attrs;
                m.vis = vis;
                methods.push(m);
            } else if content.peek(Token![type]) {
                let mut t = content.parse::<syn::ImplItemType>()?;
                t.attrs = attrs;
                t.vis = vis;
                types.push(t);
            } else if content.peek(Token![const]) {
                let mut c = content.parse::<syn::ImplItemConst>()?;
                c.attrs = attrs;
                c.vis = vis;
                consts.push(c);
            } else {
                let ident: syn::Ident = if content.peek(Token![_]) {
                    content.call(syn::ext::IdentExt::parse_any)
                } else {
                    content.parse()
                }?;
                let field = syn::Field {
                    attrs,
                    vis,
                    ident: Some(ident.clone()),
                    colon_token: Some(content.parse()?),
                    ty: content.parse()?,
                };
                let prop = Property::parse(field, pod)?;
                let name = prop.name();
                if properties.contains_key(&name) {
                        return Err(syn::Error::new_spanned(
                            &ident,
                            format!("Duplicate definition for property `{}`", name),
                        ));
                }
                properties.insert(name, prop);
                if content.is_empty() {
                    break;
                }
                content.parse::<Token![,]>()?;
            }
        }

        let properties = properties.into_values().collect();
        let signals = signals.into_values().collect();

        Ok(ObjectDefinition {
            attrs,
            vis,
            definition,
            generics,
            properties,
            signals,
            methods,
            types,
            consts,
        })
    }
}

pub enum OutputMethods {
    Type(syn::Type),
    Trait(TokenStream, syn::Generics)
}

pub struct Output {
    pub private_impl_methods: Vec<TokenStream>,
    pub define_methods: TokenStream,
    pub prop_set_impls: Vec<TokenStream>,
    pub prop_get_impls: Vec<TokenStream>,
    pub prop_defs: TokenStream,
    pub signal_defs: TokenStream,
}

impl Output {
    pub fn new(
        signals: &Vec<Signal>,
        properties: &Vec<Property>,
        method_type: OutputMethods,
        trait_name: &TokenStream,
        public_trait: Option<&syn::Ident>,
        private_trait: Option<&syn::Ident>,
        go: &syn::Ident,
    ) -> Self {
        let glib = quote! { #go::glib };

        let mut private_impl_methods = vec![];
        let mut private_prototypes = vec![];
        let mut private_methods = vec![];
        let mut public_prototypes = vec![];
        let mut public_methods = vec![];

        let signal_defs = if signals.is_empty() {
            quote! { &[] }
        } else {
            let signals = signals
                .iter()
                .map(|signal| signal.create(&glib));
            quote! {
                static SIGNALS: #glib::once_cell::sync::Lazy<::std::vec::Vec<#glib::subclass::Signal>> = #glib::once_cell::sync::Lazy::new(|| {
                    vec![
                        #(#signals),*
                    ]
                });
                ::core::convert::AsRef::as_ref(::std::ops::Deref::deref(&SIGNALS))
            }
        };

        for (index, signal) in signals.iter().enumerate() {
            let (prototypes, methods) = if signal.emit_public {
                (&mut public_prototypes, &mut public_methods)
            } else {
                (&mut private_prototypes, &mut private_methods)
            };
            prototypes.push(make_stmt(signal.emit_prototype(&glib)));
            methods.push(signal.emit_definition(index, &trait_name, &glib));

            let (prototypes, methods) = if signal.public {
                (&mut public_prototypes, &mut public_methods)
            } else {
                (&mut private_prototypes, &mut private_methods)
            };
            prototypes.push(make_stmt(signal.signal_prototype(&glib)));
            methods.push(signal.signal_definition(index, &trait_name, &glib));
            prototypes.push(make_stmt(signal.connect_prototype(&glib)));
            methods.push(signal.connect_definition(index, &trait_name, &glib));

            if let Some(method) = signal.handler_definition() {
                private_impl_methods.push(method);
            }
        }

        let mut props = vec![];
        let mut prop_set_impls = vec![];
        let mut prop_get_impls = vec![];
        for (index, prop) in properties.iter().filter(|p| !p.skip).enumerate() {
            props.push(prop.create(&go));
            let index = index + 1;
            let trait_name =  if matches!(method_type, OutputMethods::Type(_)) {
                None
            } else if prop.public {
                public_trait
            } else {
                private_trait
            };
            if let Some(set) = prop.set_impl(index, trait_name, &go) {
                prop_set_impls.push(set);
            }
            if let Some(get) = prop.get_impl(index, trait_name, &glib) {
                prop_get_impls.push(get);
            }
        }

        let prop_defs = if props.is_empty() {
            quote! { &[] }
        } else {
            quote! {
                static PROPS: #glib::once_cell::sync::Lazy<::std::vec::Vec<#glib::ParamSpec>> = #glib::once_cell::sync::Lazy::new(|| {
                    vec![#(#props),*]
                });
                ::core::convert::AsRef::as_ref(::std::ops::Deref::deref(&PROPS))
            }
        };

        for (index, prop) in properties.iter().enumerate() {
            if prop.skip {
                continue;
            }
            {
                let (prototypes, methods) = if prop.notify_public {
                    (&mut public_prototypes, &mut public_methods)
                } else {
                    (&mut private_prototypes, &mut private_methods)
                };
                prototypes.push(make_stmt(prop.notify_prototype()));
                methods.push(prop.notify_definition(index, &trait_name, &glib));
            }
            let (prototypes, methods) = if prop.public {
                (&mut public_prototypes, &mut public_methods)
            } else {
                (&mut private_prototypes, &mut private_methods)
            };
            prototypes.push(make_stmt(prop.pspec_prototype(&glib)));
            methods.push(prop.pspec_definition(index, &trait_name, &glib));
            prototypes.push(make_stmt(prop.connect_prototype(&glib)));
            methods.push(prop.connect_definition(&glib));
            if let Some(getter) = prop.getter_prototype(&go) {
                prototypes.push(make_stmt(getter));
                methods.push(prop.getter_definition(&go).expect("no getter definition"));
            }
            if let Some(setter) = prop.setter_prototype(&go) {
                prototypes.push(make_stmt(setter));
                methods.push(
                    prop.setter_definition(index, &trait_name, &go)
                        .expect("no setter definition"),
                );
            }
        }

        let define_methods = match method_type {
            OutputMethods::Type(type_) => quote! {
                impl #type_ {
                    #(#private_methods)*
                    #(pub #public_methods)*
                }
            },
            OutputMethods::Trait(type_, generics) => {
                let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
                quote! {
                    trait #private_trait {
                        #(#private_prototypes)*
                    }
                    impl #impl_generics #private_trait for #type_ #ty_generics #where_clause {
                        #(#private_methods)*
                    }
                    pub trait #public_trait {
                        #(#public_prototypes)*
                    }
                    impl #impl_generics #public_trait for #type_ #ty_generics #where_clause {
                        #(#public_methods)*
                    }
                }
            }
        };

        Self {
            private_impl_methods,
            define_methods,
            prop_set_impls,
            prop_get_impls,
            prop_defs,
            signal_defs,
        }
    }
}

