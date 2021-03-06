use proc_macro2::TokenStream;
use quote::quote;

use super::util::*;

pub struct InterfaceImplArgs(Args);

impl syn::parse::Parse for InterfaceImplArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self(Args::parse(input, true)?))
    }
}

pub fn interface_impl(args: InterfaceImplArgs, item: syn::ItemImpl) -> syn::Result<TokenStream> {
    let Args {
        type_,
        inheritance,
        pod,
    } = args.0;

    let type_ = type_.expect("no type");

    let mut def = ObjectDefinition::new(item, pod, true)?;

    let go = go_crate_ident();
    let glib = quote! { #go::glib };

    let (has_signals, signals_ident) = has_method(&def.item.items, "signals");
    let (has_properties, properties_ident) = has_method(&def.item.items, "properties");

    let (signals_path, properties_path) = {
        let self_ty = &def.item.self_ty;
        (
            if has_signals {
                quote! { #self_ty::#signals_ident }
            } else {
                quote! { <#self_ty as #glib::subclass::prelude::ObjectInterface>::#signals_ident }
            },
            if has_properties {
                quote! { #self_ty::#properties_ident }
            } else {
                quote! { <#self_ty as #glib::subclass::prelude::ObjectInterface>::#properties_ident }
            },
        )
    };

    let Output {
        mut private_impl_methods,
        prop_defs,
        public_methods,
        ..
    } = Output::new(
        &mut def,
        Some(&type_),
        &inheritance,
        &signals_path,
        &properties_path,
        &go,
    );

    let ObjectDefinition { mut item, .. } = def;

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

    Ok(quote! {
        #item
        impl #impl_generics #self_ty #ty_generics #where_clause {
            #(#private_impl_methods)*
        }
        #public_methods
    })
}
