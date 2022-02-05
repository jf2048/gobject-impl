use proc_macro2::TokenStream;
use proc_macro_error::abort;
use quote::{format_ident, quote};

use super::util::*;

pub struct ObjectImplArgs(Args);

impl syn::parse::Parse for ObjectImplArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self(Args::parse(input, false)?))
    }
}

pub fn object_impl(args: ObjectImplArgs, item: proc_macro::TokenStream) -> TokenStream {
    let Args {
        type_,
        impl_trait,
        public_trait,
        private_trait,
        pod,
    } = args.0;

    if type_.is_some() {
        if let Some(public_trait) = &public_trait {
            abort!(public_trait, "`public_trait` not allowed with `type`",);
        }
        if let Some(private_trait) = &private_trait {
            abort!(private_trait, "`private_trait` not allowed with `type`",);
        }
    }

    let definition = syn::parse::Parser::parse(
        constrain(|item| ObjectDefinition::parse(item, pod, false)),
        item,
    )
    .unwrap_or_else(|e| abort!(e));
    let header = definition.header_tokens();

    let ObjectDefinition {
        definition,
        generics,
        properties,
        signals,
        items,
        ..
    } = definition;

    let ident = match &definition {
        DefinitionType::Object { ident } => ident,
        _ => unreachable!(),
    };

    let go = go_crate_ident();
    let glib = quote! { #go::glib };

    let impl_trait_name =
        impl_trait.map(|c| c.unwrap_or_else(|| format_ident!("{}CustomObjectImplExt", ident)));
    let impl_trait = impl_trait_name.as_ref().map(|impl_trait_name| {
        quote! {
            trait #impl_trait_name: #glib::subclass::types::ObjectSubclass + #glib::subclass::object::ObjectImpl {
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

    let method_type = type_.map(OutputMethods::Type).unwrap_or_else(|| {
        OutputMethods::Trait(
            quote! { <#ident as #glib::subclass::types::ObjectSubclass>::Type },
            generics.clone(),
        )
    });

    let Output {
        private_impl_methods,
        define_methods,
        prop_set_impls,
        prop_get_impls,
        prop_defs,
        signal_defs,
    } = Output::new(
        &signals,
        &properties,
        method_type,
        &trait_name,
        Some(&public_trait),
        Some(&private_trait),
        &go,
    );

    let fields = properties.iter().filter_map(|p| p.field.as_ref());
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        #header {
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
                        id,
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
                        id,
                        pspec.name(),
                        pspec.type_().name(),
                        obj.type_().name()
                    )
                }
            }
            #(#items)*
        }
        impl #impl_generics #ident #ty_generics #where_clause {
            #(#private_impl_methods)*
        }
        #define_methods
    }
}
