use proc_macro2::TokenStream;
use proc_macro_error::{abort, abort_call_site};
use quote:: quote;

use super::util::*;

pub fn interface_impl(args: Args, item: proc_macro::TokenStream) -> TokenStream {
    let Args {
        type_,
        impl_trait,
        public_trait,
        private_trait,
        pod,
    } = args;

    if matches!(impl_trait, Some(None)) {
        abort_call_site!("`impl_trait` attribute must specify a type");
    }
    if type_.is_none() && (public_trait.is_none() || private_trait.is_none()) {
        abort_call_site!("must provide either a `type` attribute, or `public_trait` and `private_trait` attributes");
    }

    let definition = syn::parse::Parser::parse(super::constrain(|item| ObjectDefinition::parse(item, pod, false)), item)
        .unwrap_or_else(|e| abort!(e));
    let header = definition.header_tokens();

    let ObjectDefinition {
        attrs,
        vis,
        definition,
        generics,
        properties,
        signals,
        methods,
        types,
        consts,
    } = definition;

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let self_ty = match &definition {
        DefinitionType::Interface { self_ty, .. } => self_ty,
        _ => unreachable!(),
    };

    let go = super::go_crate_ident();
    let glib = quote! { #go::glib };

    let impl_trait_name = impl_trait.flatten();
    let header = if let Some(impl_trait_name) = &impl_trait_name {
        quote! {
            #(#attrs)*
            #vis impl #impl_generics #impl_trait_name for #self_ty #ty_generics #where_clause
        }
    } else {
        header
    };
    let impl_trait = impl_trait_name.as_ref().map(|impl_trait_name| {
        quote! {
            trait #impl_trait_name: #glib::subclass::prelude::ObjectInterface {
                fn properties() -> &'static [#glib::ParamSpec];
                fn signals() -> &'static [#glib::subclass::Signal];
            }
        }
    });
    let trait_name = if impl_trait.is_some() {
        quote! { #impl_trait_name }
    } else {
        quote! { #glib::subclass::prelude::ObjectInterface }
    };

    let method_type = type_
        .map(OutputMethods::Type)
        .unwrap_or_else(|| OutputMethods::Trait(
                quote! { <#self_ty as #glib::subclass::prelude::ObjectInterface>::Type },
                generics.clone()
            ));

    let Output {
        private_impl_methods,
        define_methods,
        prop_defs,
        signal_defs,
        ..
    } = Output::new(
        &signals,
        &properties,
        method_type,
        &trait_name,
        public_trait.as_ref(),
        private_trait.as_ref(),
        &go
    );

    quote! {
        #impl_trait
        #header {
            fn properties() -> &'static [#glib::ParamSpec] {
                #prop_defs
            }
            fn signals() -> &'static [#glib::subclass::Signal] {
                #signal_defs
            }
            #(#methods)*
            #(#types)*
            #(#consts)*
        }
        impl #impl_generics #self_ty #ty_generics #where_clause {
            #(#private_impl_methods)*
        }
        #define_methods
    }
}
