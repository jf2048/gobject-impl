use proc_macro2::TokenStream;
use quote::quote;

use super::util::*;

pub struct ObjectImplArgs(Args);

impl syn::parse::Parse for ObjectImplArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self(Args::parse(input, false)?))
    }
}

pub fn object_impl(args: ObjectImplArgs, item: syn::ItemImpl) -> syn::Result<TokenStream> {
    let Args {
        type_,
        inheritance,
        pod,
    } = args.0;

    let def = ObjectDefinition::new(item, pod, false)?;

    let go = go_crate_ident();
    let glib = quote! { #go::glib };

    let (has_signals, signals_ident) = has_method(&def.item.items, "signals");
    let (has_properties, properties_ident) = has_method(&def.item.items, "properties");
    let (has_set_property, set_property_ident) = has_method(&def.item.items, "set_property");
    let (has_property, property_ident) = has_method(&def.item.items, "property");

    let (signals_path, properties_path) = {
        let self_ty = &def.item.self_ty;
        (
            if has_signals {
                quote! { #self_ty::#signals_ident }
            } else {
                quote! { <#self_ty as #glib::subclass::object::ObjectImpl>::#signals_ident }
            },
            if has_properties {
                quote! { #self_ty::#properties_ident }
            } else {
                quote! { <#self_ty as #glib::subclass::object::ObjectImpl>::#properties_ident }
            },
        )
    };

    let Output {
        mut private_impl_methods,
        prop_set_impls,
        prop_get_impls,
        prop_defs,
        signal_defs,
        public_methods,
    } = Output::new(
        &def,
        type_.as_ref(),
        &inheritance,
        &signals_path,
        &properties_path,
        &go,
    );

    let ObjectDefinition {
        mut item,
        struct_item,
        ..
    } = def;

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
        let set_property_def = quote! {
            fn #set_property_ident(
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
                        id,
                        pspec.name(),
                        pspec.type_().name(),
                        <<Self as #glib::subclass::types::ObjectSubclass>::Type as #go::glib::object::ObjectExt>::type_(
                            obj
                        ).name()
                    )
                }
            }
        };
        let property_def = quote! {
            fn #property_ident(
                &self,
                obj: &<Self as #glib::subclass::types::ObjectSubclass>::Type,
                id: usize,
                pspec: &#glib::ParamSpec
            ) -> #glib::Value {
                match id {
                    #(#prop_get_impls)*
                    _ => unimplemented!(
                        "invalid property id {} for \"{}\" of type '{}' in '{}'",
                        id,
                        pspec.name(),
                        pspec.type_().name(),
                        <<Self as #glib::subclass::types::ObjectSubclass>::Type as #go::glib::object::ObjectExt>::type_(
                            obj
                        ).name()
                    )
                }
            }
        };
        if has_properties {
            private_impl_methods.push(properties_def);
        } else {
            item.items.push(syn::ImplItem::Verbatim(properties_def));
        }
        if has_set_property {
            private_impl_methods.push(set_property_def);
        } else {
            item.items.push(syn::ImplItem::Verbatim(set_property_def));
        }
        if has_property {
            private_impl_methods.push(property_def);
        } else {
            item.items.push(syn::ImplItem::Verbatim(property_def));
        }
    }

    let self_ty = &item.self_ty;
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();

    Ok(quote! {
        #struct_item
        #item
        impl #impl_generics #self_ty #ty_generics #where_clause {
            #(#private_impl_methods)*
        }
        #public_methods
    })
}
