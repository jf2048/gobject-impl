use proc_macro2::TokenStream;
use proc_macro_error::{abort, abort_call_site};
use quote::quote;

use super::util::*;

pub struct InterfaceImplArgs(Args);

impl syn::parse::Parse for InterfaceImplArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self(Args::parse(input, true)?))
    }
}

pub fn interface_impl(args: InterfaceImplArgs, item: syn::ItemImpl) -> TokenStream {
    let Args {
        type_,
        trait_,
        pod,
    } = args.0;

    let type_ = type_.unwrap_or_else(|| {
        abort_call_site!("`type` attribute required for `interface_impl`");
    });

    let definition = ObjectDefinition::new(item, pod, true)
        .unwrap_or_else(|e| abort!(e));

    let ObjectDefinition {
        mut item,
        properties,
        signals,
        ..
    } = definition;

    let go = go_crate_ident();
    let glib = quote! { #go::glib };

    let (has_signals, signals_ident) = has_method(&item.items, "signals");
    let (has_properties, properties_ident) = has_method(&item.items, "properties");

    let subclass = quote! { <Self as #glib::ObjectType>::GlibClassType };
    let signals_path = if has_signals {
        quote! { #subclass::#signals_ident }
    } else {
        quote! { <#subclass as #glib::subclass::prelude::ObjectInterface>::#signals_ident }
    };
    let properties_path = if has_properties {
        quote! { #subclass::#properties_ident }
    } else {
        quote! { <#subclass as #glib::subclass::prelude::ObjectInterface>::#properties_ident }
    };

    let Output {
        mut private_impl_methods,
        prop_defs,
        signal_defs,
        ext_trait,
        ..
    } = Output::new(
        &item,
        &signals,
        &properties,
        Some(&type_),
        Some(&trait_),
        &signals_path,
        &properties_path,
        &go,
    );

    if let Some(signal_defs) = &signal_defs {
        let signals_def = quote! {
            fn #signals_ident() -> &'static [#glib::subclass::Signal] {
                #signal_defs
            }
        };
        if has_signals {
            private_impl_methods.push(signals_def);
        } else {
            item.items.push(syn::ImplItem::Verbatim(signals_def));
        }
    }

    if let Some(prop_defs) = &prop_defs {
        let properties_def = quote! {
            fn #properties_ident() -> &'static [#glib::ParamSpec] {
                #prop_defs
            }
        };
        if has_properties {
            private_impl_methods.push(properties_def);
        } else {
            item.items.push(syn::ImplItem::Verbatim(properties_def));
        }
    }

    let self_ty = &item.self_ty;
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();

    quote! {
        #item
        impl #impl_generics #self_ty #ty_generics #where_clause {
            #(#private_impl_methods)*
        }
        #ext_trait
    }
}
