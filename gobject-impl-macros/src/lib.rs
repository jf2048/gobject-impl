use heck::ToKebabCase;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro_error::{abort_call_site, proc_macro_error};
use quote::{format_ident, quote, ToTokens};
use std::collections::HashMap;
use syn::{parse::Parse, Token};

mod property;
use property::*;
mod signal;
use signal::*;

fn go_crate_ident() -> syn::Ident {
    use proc_macro_crate::FoundCrate;

    let crate_name = match proc_macro_crate::crate_name("gobject-impl") {
        Ok(FoundCrate::Name(name)) => name,
        _ => "gobject-impl".to_owned(),
    };

    syn::Ident::new(&crate_name, proc_macro2::Span::call_site())
}

mod keywords {
    // struct attributes
    syn::custom_keyword!(impl_trait);
    syn::custom_keyword!(public_trait);
    syn::custom_keyword!(private_trait);
    syn::custom_keyword!(pod);

    // methods
    syn::custom_keyword!(constructed);
    syn::custom_keyword!(dispose);
}

struct Args {
    pub impl_trait: Option<Option<syn::Ident>>,
    pub public_trait: Option<syn::Ident>,
    pub private_trait: Option<syn::Ident>,
    pub pod: bool,
}

impl Parse for Args {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut args = Self {
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
            if lookahead.peek(keywords::impl_trait) {
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
        Ok(args)
    }
}

enum MethodName {
    Constructed,
    Dispose,
}

struct Method {
    pub attrs: Vec<syn::Attribute>,
    pub inputs: Vec<syn::FnArg>,
    pub block: Box<syn::Block>,
}

impl Method {
    fn to_tokens(&self, name: &str) -> TokenStream2 {
        let Self {
            attrs,
            inputs,
            block,
        } = self;
        let name = format_ident!("{}", name);
        let args = inputs.iter().skip(1);
        quote! {
            #(#attrs)*
            fn #name(&self, #(#args),*) {
                #block
            }
        }
    }
}

struct CustomStruct {
    pub attrs: Vec<syn::Attribute>,
    pub vis: syn::Visibility,
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    pub properties: Vec<Property>,
    pub signals: HashMap<String, Signal>,
    pub constructed: Option<Method>,
    pub dispose: Option<Method>,
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

impl CustomStruct {
    fn parse(input: syn::parse::ParseStream, pod: bool) -> syn::Result<Self> {
        let mut attrs = input.call(syn::Attribute::parse_outer)?;
        let vis = input.parse::<syn::Visibility>()?;
        input.parse::<Token![struct]>()?;
        let ident = input.parse::<syn::Ident>()?;
        let generics = input.parse::<syn::Generics>()?;
        let mut where_clause = None;
        if input.peek(Token![where]) {
            where_clause = Some(input.parse()?);
        }

        let content;
        syn::braced!(content in input);
        while content.peek(Token![#]) && content.peek2(Token![!]) {
            attrs.push({
                let attr_content;
                let bracket_token = syn::bracketed!(attr_content in content);
                let path = attr_content.call(syn::Path::parse_mod_style)?;
                let tokens = attr_content.parse()?;
                syn::Attribute {
                    pound_token: content.parse()?,
                    style: syn::AttrStyle::Inner(content.parse()?),
                    bracket_token,
                    path,
                    tokens,
                }
            });
        }
        let mut properties = vec![];
        let mut signals = HashMap::<String, Signal>::new();
        let mut constructed = None;
        let mut dispose = None;

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

                let signal = signals.entry(name.to_owned()).or_default();
                if signal.inputs.is_some() {
                    return Err(syn::Error::new_spanned(
                        ident,
                        format!("Duplicate definition for signal `{}`", name),
                    ));
                }
                if let Some(attrs) = signal_attrs {
                    signal.flags = attrs.flags;
                    signal.emit_public = attrs.emit_public;
                }
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
                let mut acc_attrs = None;
                for attr in attrs {
                    if acc_attrs.is_some() {
                        return Err(syn::Error::new_spanned(
                            attr,
                            "Only one attribute allowed on signal_accumulator",
                        ));
                    }
                    acc_attrs = Some(syn::parse2::<SignalAccumulatorAttrs>(attr.tokens)?);
                }
                let name = acc_attrs
                    .as_ref()
                    .and_then(|a| a.name.to_owned())
                    .unwrap_or_else(|| ident.to_string().to_kebab_case());
                let inputs = parse_args(&content)?;
                let output = content.parse::<syn::ReturnType>()?;
                if matches!(output, syn::ReturnType::Default) {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "signal_accumulator must have return type",
                    ));
                }
                let block = Box::new(input.parse::<syn::Block>()?);

                let signal = signals.entry(name.to_owned()).or_default();
                if signal.accumulator.is_some() {
                    return Err(syn::Error::new_spanned(
                        ident,
                        format!(
                            "Duplicate definition for signal_accumulator on signal `{}`",
                            name
                        ),
                    ));
                }
                signal.accumulator = Some((kw, inputs, block));
            } else if content.peek(Token![fn]) {
                if !matches!(vis, syn::Visibility::Inherited) {
                    return Err(syn::Error::new_spanned(
                        vis,
                        "function cannot have visibility",
                    ));
                }
                content.parse::<Token![fn]>()?;
                let (kw, method_name) = if content.peek(keywords::constructed) {
                    (
                        content
                            .parse::<keywords::constructed>()?
                            .into_token_stream(),
                        MethodName::Constructed,
                    )
                } else if content.peek(keywords::dispose) {
                    (
                        content.parse::<keywords::dispose>()?.into_token_stream(),
                        MethodName::Dispose,
                    )
                } else {
                    let ident: syn::Ident = content.call(syn::ext::IdentExt::parse_any)?;
                    return Err(syn::Error::new_spanned(
                        ident.clone(),
                        format!("Unknown ObjectImpl function `{}`", ident),
                    ));
                };
                let inputs = parse_args(&content)?;
                let recv = inputs.get(0).and_then(|a| match a {
                    syn::FnArg::Receiver(r) => Some(r),
                    _ => None,
                });
                if recv.is_none() {
                    let span = inputs
                        .get(0)
                        .map(|t| t.to_token_stream())
                        .unwrap_or_else(|| kw);
                    return Err(syn::Error::new_spanned(
                        span,
                        "First argument to function must be `&self`",
                    ));
                }
                let block = Box::new(input.parse::<syn::Block>()?);
                let storage = match method_name {
                    MethodName::Constructed => &mut constructed,
                    MethodName::Dispose => &mut dispose,
                };
                if storage.is_some() {
                    return Err(syn::Error::new_spanned(
                        kw.clone(),
                        format!("Duplicate definition for `{}`", kw),
                    ));
                }
                storage.replace(Method {
                    attrs,
                    inputs,
                    block,
                });
            } else {
                let ident = Some(if content.peek(Token![_]) {
                    content.call(syn::ext::IdentExt::parse_any)
                } else {
                    content.parse()
                }?);
                let field = syn::Field {
                    attrs,
                    vis,
                    ident,
                    colon_token: Some(content.parse()?),
                    ty: content.parse()?,
                };
                let prop = Property::parse(field, pod)?;
                properties.push(prop);
            }
            if content.is_empty() {
                break;
            }
            content.parse::<Token![,]>()?;
        }

        Ok(CustomStruct {
            attrs,
            vis,
            ident,
            generics: syn::Generics {
                where_clause,
                ..generics
            },
            properties,
            signals,
            constructed,
            dispose,
        })
    }
}

#[inline]
fn constrain<F, T>(f: F) -> F
where
    F: for<'r> Fn(&'r syn::parse::ParseBuffer<'r>) -> syn::Result<T>,
{
    f
}

#[inline]
fn make_stmt(tokens: TokenStream2) -> TokenStream2 {
    quote! { #tokens; }
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn object_impl(attr: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let Args {
        impl_trait,
        public_trait,
        private_trait,
        pod,
    } = syn::parse_macro_input!(attr as Args);

    let CustomStruct {
        attrs,
        vis,
        ident,
        generics,
        properties,
        signals,
        constructed,
        dispose,
    } = syn::parse::Parser::parse(constrain(|item| CustomStruct::parse(item, pod)), item)
        .unwrap_or_else(|_| {
            abort_call_site!(
                "This macro should be used on the `struct` statement for an object impl"
            )
        });
    let go = go_crate_ident();
    let glib = quote! { #go::glib };
    let impl_trait_name =
        impl_trait.map(|c| c.unwrap_or_else(|| format_ident!("{}CustomObjectImplExt", ident)));
    let impl_trait = impl_trait_name.as_ref().map(|impl_trait_name| {
        quote! {
            trait #impl_trait_name {
                fn properties() -> &'static [#glib::ParamSpec];
                fn signals() -> &'static [#glib::subclass::Signal];
                fn set_property(&self, obj: &<Self as #glib::subclass::types::ObjectSubclass>::Type, _id: usize, _value: &#glib::Value, _pspec: &#glib::ParamSpec);
                fn property(&self, obj: &<Self as #glib::subclass::types::ObjectSubclass>::Type, _id: usize, _pspec: &#glib::ParamSpec) -> #glib::Value;
                fn constructed(&self, obj: &<Self as #glib::subclass::types::ObjectSubclass>::Type) {
                    <Self as #glib::subclass::object::ObjectImplExt>::parent_constructed(self, obj);
                }
                fn dispose(&self, _obj: &<Self as #glib::subclass::types::ObjectSubclass>::Type) {}
            }
        }
    });
    let trait_name = if impl_trait.is_some() {
        quote! { #impl_trait_name }
    } else {
        quote! { #glib::subclass::object::ObjectImpl }
    };

    let public_trait = public_trait.unwrap_or_else(|| format_ident!("{}ObjectExt", ident));
    let private_trait = private_trait.unwrap_or_else(|| format_ident!("{}ObjectImplExt", ident));
    let mut private_impl_methods = vec![];
    let mut private_prototypes = vec![];
    let mut private_methods = vec![];
    let mut public_prototypes = vec![];
    let mut public_methods = vec![];

    let constructed = constructed.map(|m| m.to_tokens("constructed"));
    let dispose = dispose.map(|m| m.to_tokens("dispose"));

    let signal_defs = if signals.is_empty() {
        quote! { &[] }
    } else {
        let signals = signals
            .iter()
            .map(|(name, signal)| signal.create(name, &glib));
        quote! {
            static SIGNALS: #glib::once_cell::sync::Lazy<::std::vec::Vec<#glib::subclass::Signal>> = #glib::once_cell::sync::Lazy::new(|| {
                vec![
                    #(#signals),*
                ]
            });
            <#glib::once_cell::sync::Lazy<::std::vec::Vec<#glib::subclass::Signal>> as ::core::convert::AsRef>::as_ref(&SIGNALS)
        }
    };

    for (index, (name, signal)) in signals.iter().enumerate() {
        let (prototypes, methods) = if signal.emit_public {
            (&mut public_prototypes, &mut public_methods)
        } else {
            (&mut private_prototypes, &mut private_methods)
        };
        prototypes.push(make_stmt(signal.emit_prototype(name, &glib)));
        methods.push(signal.emit_definition(index, name, &trait_name, &glib));

        let (prototypes, methods) = if signal.public {
            (&mut public_prototypes, &mut public_methods)
        } else {
            (&mut private_prototypes, &mut private_methods)
        };
        prototypes.push(make_stmt(signal.signal_prototype(name, &glib)));
        methods.push(signal.signal_definition(index, name, &trait_name, &glib));
        prototypes.push(make_stmt(signal.connect_prototype(name, &glib)));
        methods.push(signal.connect_definition(index, name, &trait_name, &glib));

        if let Some(method) = signal.handler_definition(name) {
            private_impl_methods.push(method);
        }
    }

    let mut props = vec![];
    let mut prop_set_impls = vec![];
    let mut prop_get_impls = vec![];
    for (index, prop) in properties.iter().filter(|p| !p.skip).enumerate() {
        props.push(prop.create(&go));
        let index = index + 1;
        let trait_name = if prop.public {
            &public_trait
        } else {
            &private_trait
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
            <#glib::once_cell::sync::Lazy<::std::vec::Vec<#glib::ParamSpec>> as ::core::convert::AsRef>::as_ref(&PROPS)
        }
    };

    for (index, prop) in properties.iter().enumerate() {
        if prop.skip {
            continue;
        }
        let (prototypes, methods) = if prop.public {
            (&mut public_prototypes, &mut public_methods)
        } else {
            (&mut private_prototypes, &mut private_methods)
        };
        prototypes.push(make_stmt(prop.pspec_prototype(&glib)));
        methods.push(prop.pspec_definition(index, &trait_name, &glib));
        prototypes.push(make_stmt(prop.notify_prototype()));
        methods.push(prop.notify_definition(index, &trait_name, &glib));
        prototypes.push(make_stmt(prop.connect_prototype(&glib)));
        methods.push(prop.connect_definition(&glib));
        if let Some(getter) = prop.getter_prototype() {
            prototypes.push(getter);
            methods.push(prop.getter_definition(&go).unwrap());
        }
        if let Some(setter) = prop.setter_prototype() {
            prototypes.push(setter);
            methods.push(prop.setter_definition(index, &trait_name, &go).unwrap());
        }
    }

    let fields = properties.iter().filter_map(|p| p.field.as_ref());
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        #(#attrs)*
        #vis struct #ident #generics {
            #(#fields),*
        }
        #impl_trait
        impl #trait_name for #ident {
            fn properties() -> &'static [#glib::ParamSpec] {
                #prop_defs
            }
            fn signals() -> &'static [#glib::subclass::Signal] {
                #signal_defs
            }
            fn set_property(
                &self,
                obj: &<Self as #glib::subclass::types::ObjectSubclass>::Type,
                id: usize,
                value: &#glib::Value,
                pspec: &#glib::ParamSpec
            ) {
                match id {
                    #(#prop_set_impls)*
                    _ => unimplemented!(
                        "invalid property id {} for \"{}\" of type '{}' in '{}'",
                        index,
                        pspec.name(),
                        pspec.type_().name(),
                        obj.type_().name()
                    )
                }
            }
            fn property(
                &self,
                obj: &<Self as #glib::subclass::types::ObjectSubclass>::Type,
                id: usize,
                pspec: &#glib::ParamSpec
            ) -> #glib::Value {
                match id {
                    #(#prop_get_impls)*
                    _ => unimplemented!(
                        "invalid property id {} for \"{}\" of type '{}' in '{}'",
                        index,
                        pspec.name(),
                        pspec.type_().name(),
                        obj.type_().name()
                    )
                }
            }
            #constructed
            #dispose
        }
        impl #impl_generics #ident #ty_generics #where_clause {
            #(#private_impl_methods)*
        }
        trait #private_trait {
            #(#private_prototypes)*
        }
        impl #impl_generics #private_trait for #ident #ty_generics #where_clause {
            #(#private_methods)*
        }
        pub trait #public_trait {
            #(#public_prototypes)*
        }
        impl #impl_generics #public_trait for #ident #ty_generics #where_clause {
            #(#public_methods)*
        }
    }
    .into()
}
