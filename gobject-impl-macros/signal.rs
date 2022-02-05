use heck::ToSnakeCase;
use proc_macro2::TokenStream;
use proc_macro_error::abort;
use quote::{format_ident, quote};
use syn::{parse::Parse, Token};

pub mod keywords {
    // signal keywords
    syn::custom_keyword!(signal);
    syn::custom_keyword!(accumulator);

    // signal attributes
    syn::custom_keyword!(name);
    syn::custom_keyword!(emit);
    syn::custom_keyword!(run_first);
    syn::custom_keyword!(run_last);
    syn::custom_keyword!(run_cleanup);
    syn::custom_keyword!(no_recurse);
    syn::custom_keyword!(detailed);
    syn::custom_keyword!(action);
    syn::custom_keyword!(no_hooks);
    syn::custom_keyword!(must_collect);
    syn::custom_keyword!(deprecated);
}

bitflags::bitflags! {
    pub struct SignalFlags: u32 {
        const RUN_FIRST    = 0b000000001;
        const RUN_LAST     = 0b000000010;
        const RUN_CLEANUP  = 0b000000100;
        const NO_RECURSE   = 0b000001000;
        const DETAILED     = 0b000010000;
        const ACTION       = 0b000100000;
        const NO_HOOKS     = 0b001000000;
        const MUST_COLLECT = 0b010000000;
        const DEPRECATED   = 0b100000000;
    }
}

impl SignalFlags {
    fn tokens(&self, glib: &TokenStream) -> TokenStream {
        let count = Self::empty().bits().leading_zeros() - Self::all().bits().leading_zeros();
        let mut flags = vec![];
        for i in 0..count {
            if let Some(flag) = Self::from_bits(1 << i) {
                if self.contains(flag) {
                    let flag = format!("{:?}", flag);
                    let flag = format_ident!("{}", flag);
                    flags.push(quote! { #glib::SignalFlags::#flag });
                }
            }
        }
        if flags.is_empty() {
            quote! { #glib::SignalFlags::empty() }
        } else {
            quote! { #(#flags)|* }
        }
    }
}

pub struct SignalAttrs {
    pub flags: SignalFlags,
    pub emit_public: bool,
    pub name: Option<String>,
}

impl Parse for SignalAttrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = Self {
            flags: SignalFlags::empty(),
            emit_public: false,
            name: None,
        };

        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if lookahead.peek(keywords::emit) {
                let kw = input.parse::<keywords::emit>()?;
                if attrs.emit_public {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `emit` attribute"));
                }
                input.parse::<Token![=]>()?;
                input.parse::<Token![pub]>()?;
                attrs.emit_public = true;
            } else if lookahead.peek(keywords::name) {
                let kw = input.parse::<keywords::name>()?;
                if attrs.name.is_some() {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `name` attribute"));
                }
                input.parse::<Token![=]>()?;
                attrs.name = Some(input.parse::<syn::LitStr>()?.value());
            } else {
                use keywords::*;

                macro_rules! parse_flags {
                    (@body $name:ty: $kw:expr => $flag:expr) => {
                        let kw = input.parse::<$name>()?;
                        let flag = $flag;
                        if attrs.flags.contains(flag) {
                            let msg = format!("Duplicate `{}` attribute", <$name as syn::token::CustomToken>::display());
                            return Err(syn::Error::new_spanned(kw, msg));
                        }
                        attrs.flags |= flag;
                    };
                    ($name:ty: $kw:expr => $flag:expr) => {
                        if lookahead.peek($kw) {
                            parse_flags!(@body $name: $kw => $flag);
                        } else {
                            return Err(lookahead.error());
                        }
                    };
                    ($name:ty: $kw:expr => $flag:expr, $($names:ty: $kws:expr => $flags:expr),+) => {
                        if lookahead.peek($kw) {
                            parse_flags!(@body $name: $kw => $flag);
                        } else {
                            parse_flags! { $($names: $kws => $flags),+ }
                        }
                    };
                }
                parse_flags! {
                    run_first:    run_first    => SignalFlags::RUN_FIRST,
                    run_last:     run_last     => SignalFlags::RUN_LAST,
                    run_cleanup:  run_cleanup  => SignalFlags::RUN_CLEANUP,
                    no_recurse:   no_recurse   => SignalFlags::NO_RECURSE,
                    detailed:     detailed     => SignalFlags::DETAILED,
                    action:       action       => SignalFlags::ACTION,
                    no_hooks:     no_hooks     => SignalFlags::NO_HOOKS,
                    must_collect: must_collect => SignalFlags::MUST_COLLECT,
                    deprecated:   deprecated   => SignalFlags::DEPRECATED
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(attrs)
    }
}

pub struct Signal {
    pub ident: syn::Ident,
    pub name: String,
    pub flags: SignalFlags,
    pub interface: bool,
    pub public: bool,
    pub emit_public: bool,
    pub inputs: Option<Vec<syn::FnArg>>,
    pub output: syn::ReturnType,
    pub block: Option<Box<syn::Block>>,
    pub accumulator: Option<(keywords::accumulator, Vec<syn::FnArg>, Box<syn::Block>)>,
}

impl Signal {
    pub fn new(ident: syn::Ident) -> Self {
        Self {
            ident,
            name: Default::default(),
            flags: SignalFlags::empty(),
            interface: false,
            public: false,
            emit_public: false,
            inputs: Default::default(),
            output: syn::ReturnType::Default,
            block: Default::default(),
            accumulator: Default::default(),
        }
    }
    fn inputs(&self) -> &Vec<syn::FnArg> {
        self.inputs.as_ref().unwrap_or_else(|| {
            let (acc_kw, _, _) = self.accumulator.as_ref().expect("no accumulator");
            abort!(acc_kw, format!("No definition for signal `{}`", self.name));
        })
    }
    fn impl_trait(&self, trait_name: &TokenStream, glib: &TokenStream) -> TokenStream {
        let imp = if self.interface {
            quote! { <Self as #glib::ObjectType>::GlibClassType }
        } else {
            quote! { <Self as #glib::object::ObjectSubclassIs>::Subclass }
        };
        quote! { <#imp as #trait_name> }
    }
    fn arg_names(&self) -> impl Iterator<Item = syn::Ident> + '_ {
        self.inputs()
            .iter()
            .enumerate()
            .map(|(i, _)| format_ident!("arg{}", i))
    }
    fn args_unwrap<'a>(
        &'a self,
        object_type: Option<&'a syn::Type>,
        imp: bool,
        glib: &'a TokenStream,
    ) -> impl Iterator<Item = TokenStream> + 'a {
        self.inputs().iter().enumerate().map(move |(index, input)| {
            let ty = match input {
                syn::FnArg::Receiver(_) => {
                    if imp {
                        if let Some(ty) = object_type {
                            quote! { #ty }
                        } else {
                            quote! { <Self as #glib::subclass::types::ObjectSubclass>::Type }
                        }
                    } else {
                        quote! { Self }
                    }
                }
                syn::FnArg::Typed(t) => {
                    let ty = &t.ty;
                    quote! { #ty }
                }
            };
            let arg_name = format_ident!("arg{}", index);
            let err_msg = format!("Wrong type for argument {}: {{:?}}", index);
            quote! {
                let #arg_name = args[#index].get::<#ty>().unwrap_or_else(|e| {
                    panic!(#err_msg, e)
                });
            }
        })
    }
    pub fn create(&self, object_type: Option<&syn::Type>, glib: &TokenStream) -> TokenStream {
        let Self {
            name,
            flags,
            output,
            block,
            accumulator,
            ..
        } = self;

        let inputs = self.inputs();
        let input_static_types = inputs.iter().map(|input| quote! {
            <#glib::subclass::SignalType as ::core::convert::From<#glib::types::StaticType>>::from(
                <#input as #glib::types::StaticType>::static_type()
            )
        });
        let arg_names = self.arg_names();
        let args_unwrap = self.args_unwrap(object_type, true, glib);
        let class_handler = block.is_some().then(|| {
            let method_name = self.handler_name();
            quote! {
                let builder = builder.class_handler(|_, args| {
                    #(#args_unwrap)*
                    let ret = Self::#method_name(#(#arg_names),*);
                    #glib::closure::ToClosureReturnValue::to_closure_return_value(&ret)
                });
            }
        });
        let accumulator = accumulator.as_ref().map(|(_, args, block)| {
            quote! {
                let builder = builder.accumulator(|hint, acc, value| {
                    fn ____accumulator(#(#args),*) -> bool {
                        #block
                    }
                    ____accumulator(hint, acc, value)
                });
            }
        });
        let flags = flags.tokens(glib);
        quote! {
            {
                let builder = #glib::subclass::Signal::builder(
                    #name,
                    &[#(#input_static_types),*],
                    <#glib::subclass::SignalType as ::core::convert::From<#glib::types::StaticType>>::from(
                        <#output as #glib::types::StaticType>::static_type()
                    )
                );
                let builder = builder.flags(#flags);
                #class_handler
                #accumulator
                builder.build()
            }
        }
    }
    fn handler_name(&self) -> syn::Ident {
        format_ident!("{}_class_handler", self.name.to_snake_case())
    }
    pub fn handler_definition(&self) -> Option<TokenStream> {
        if let Some(block) = &self.block {
            let Self { inputs, output, .. } = self;
            let inputs = inputs.as_ref().expect("no inputs");
            let method_name = self.handler_name();
            Some(quote! {
                fn #method_name(#(#inputs),*) #output {
                    #block
                }
            })
        } else {
            None
        }
    }
    fn emit_arg_defs(&self) -> impl Iterator<Item = syn::PatType> + Clone + '_ {
        self.inputs()
            .iter()
            .skip(1)
            .enumerate()
            .map(|(index, arg)| {
                let mut ty = match arg {
                    syn::FnArg::Typed(t) => t,
                    _ => unimplemented!(),
                }
                .clone();
                let pat_ident = Box::new(syn::Pat::Ident(syn::PatIdent {
                    attrs: vec![],
                    by_ref: None,
                    mutability: None,
                    ident: format_ident!("arg{}", index),
                    subpat: None,
                }));
                if !matches!(&*ty.pat, syn::Pat::Ident(_)) {
                    ty.pat = pat_ident;
                }
                ty
            })
    }
    pub fn signal_prototype(&self, glib: &TokenStream) -> TokenStream {
        let method_name = format_ident!("signal_{}", self.name.to_snake_case());
        quote! {
            fn #method_name(&self) -> &'static #glib::subclass::Signal
        }
    }
    pub fn signal_definition(
        &self,
        index: usize,
        trait_name: &TokenStream,
        glib: &TokenStream,
    ) -> TokenStream {
        let proto = self.signal_prototype(glib);
        let impl_trait = self.impl_trait(trait_name, glib);
        quote! {
            #proto {
                &#impl_trait::signals()[#index]
            }
        }
    }
    pub fn emit_prototype(&self, glib: &TokenStream) -> TokenStream {
        let Self { output, .. } = self;
        let method_name = format_ident!("emit_{}", self.name.to_snake_case());
        let arg_defs = self.emit_arg_defs();
        let details_arg = self
            .flags
            .contains(SignalFlags::DETAILED)
            .then(|| quote! { signal_details: ::std::option::Option<#glib::Quark>, });
        quote! {
            fn #method_name(&self, #details_arg #(#arg_defs),*) #output
        }
    }
    pub fn emit_definition(
        &self,
        index: usize,
        trait_name: &TokenStream,
        glib: &TokenStream,
    ) -> TokenStream {
        let proto = self.emit_prototype(glib);
        let arg_defs = self.emit_arg_defs();
        let arg_names = arg_defs.clone().map(|arg| match &*arg.pat {
            syn::Pat::Ident(syn::PatIdent { ident, .. }) => ident.clone(),
            _ => unimplemented!(),
        });
        let impl_trait = self.impl_trait(trait_name, glib);
        let signal_id = quote! { #impl_trait::signals()[#index].signal_id() };
        let emit = {
            let arg_names = arg_names.clone();
            quote! {
                <Self as #glib::object::ObjectExt>::emit(
                    #signal_id,
                    &[#(#arg_names),*]
                )
            }
        };
        let body = if self.flags.contains(SignalFlags::DETAILED) {
            quote! {
                if let Some(signal_details) = signal_details {
                    <Self as #glib::object::ObjectExt>::emit(
                        #signal_id,
                        signal_details,
                        &[#(#arg_names),*]
                    )
                } else {
                    #emit
                }
            }
        } else {
            emit
        };
        quote! {
            #proto {
                #glib::closure::TryFromClosureReturnValue::try_from_closure_return_value(
                    #body
                ).unwrap()
            }
        }
    }
    pub fn connect_prototype(&self, glib: &TokenStream) -> TokenStream {
        let method_name = format_ident!("connect_{}", self.name.to_snake_case());
        let Self { output, .. } = self;
        let input_types = self.inputs().iter().skip(1).map(|arg| match arg {
            syn::FnArg::Typed(t) => &t.ty,
            _ => unimplemented!(),
        });
        let details_arg = self
            .flags
            .contains(SignalFlags::DETAILED)
            .then(|| quote! { details: ::std::option::Option<#glib::Quark>, });
        quote! {
            fn #method_name<F: Fn(&Self, #(#input_types),*) #output + 'static>(
                &self,
                #details_arg
                f: F,
            ) -> #glib::SignalHandlerId
        }
    }
    pub fn connect_definition(
        &self,
        index: usize,
        trait_name: &TokenStream,
        glib: &TokenStream,
    ) -> TokenStream {
        let proto = self.connect_prototype(glib);
        let arg_names = self.arg_names();
        let args_unwrap = self.args_unwrap(None, false, glib);
        let impl_trait = self.impl_trait(trait_name, glib);

        let details = if self.flags.contains(SignalFlags::DETAILED) {
            quote! { details, }
        } else {
            quote! { ::std::option::Option::None }
        };

        quote! {
            #proto {
                self.connect_local_id(
                    #impl_trait::signals()[#index].signal_id(),
                    #details,
                    false,
                    move |args| {
                        let recv = args[0].get::<&Self>().unwrap();
                        #(#args_unwrap)*
                        let ret = f(recv, #(#arg_names),*);
                        #glib::closure::ToClosureReturnValue::to_closure_return_value(&ret)
                    },
                )
            }
        }
    }
}
