use heck::ToSnakeCase;
use proc_macro2::TokenStream;
use proc_macro_error::abort;
use quote::{format_ident, quote};
use syn::{parse::Parse, Token};

pub mod keywords {
    syn::custom_keyword!(name);
    syn::custom_keyword!(emit);
    syn::custom_keyword!(connect);
    syn::custom_keyword!(run_first);
    syn::custom_keyword!(run_last);
    syn::custom_keyword!(run_cleanup);
    syn::custom_keyword!(no_recurse);
    syn::custom_keyword!(detailed);
    syn::custom_keyword!(action);
    syn::custom_keyword!(no_hooks);
    syn::custom_keyword!(must_collect);
    syn::custom_keyword!(deprecated);
    syn::custom_keyword!(accumulator_first_run);
}

bitflags::bitflags! {
    pub struct SignalFlags: u32 {
        const RUN_FIRST             = 1 << 0;
        const RUN_LAST              = 1 << 1;
        const RUN_CLEANUP           = 1 << 2;
        const NO_RECURSE            = 1 << 3;
        const DETAILED              = 1 << 4;
        const ACTION                = 1 << 5;
        const NO_HOOKS              = 1 << 6;
        const MUST_COLLECT          = 1 << 7;
        const DEPRECATED            = 1 << 8;
        const ACCUMULATOR_FIRST_RUN = 1 << 17;
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
    pub emit: bool,
    pub connect: bool,
    pub name: Option<String>,
}

impl Parse for SignalAttrs {
    fn parse(stream: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = Self {
            flags: SignalFlags::empty(),
            emit: true,
            connect: true,
            name: None,
        };

        if stream.is_empty() {
            return Ok(attrs);
        }

        let input;
        syn::parenthesized!(input in stream);
        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if lookahead.peek(keywords::name) {
                let kw = input.parse::<keywords::name>()?;
                if attrs.name.is_some() {
                    return Err(syn::Error::new_spanned(kw, "Duplicate `name` attribute"));
                }
                input.parse::<Token![=]>()?;
                attrs.name = Some(input.parse::<syn::LitStr>()?.value());
            } else if lookahead.peek(Token![!]) {
                input.parse::<Token![!]>()?;
                let lookahead = input.lookahead1();
                if lookahead.peek(keywords::emit) {
                    let kw = input.parse::<keywords::emit>()?;
                    if !attrs.emit {
                        return Err(syn::Error::new_spanned(kw, "Duplicate `emit` attribute"));
                    }
                    attrs.emit = false;
                } else if lookahead.peek(keywords::connect) {
                    let kw = input.parse::<keywords::connect>()?;
                    if !attrs.connect {
                        return Err(syn::Error::new_spanned(kw, "Duplicate `connect` attribute"));
                    }
                    attrs.connect = false;
                } else {
                    return Err(lookahead.error());
                }
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
                    run_first:             run_first             => SignalFlags::RUN_FIRST,
                    run_last:              run_last              => SignalFlags::RUN_LAST,
                    run_cleanup:           run_cleanup           => SignalFlags::RUN_CLEANUP,
                    no_recurse:            no_recurse            => SignalFlags::NO_RECURSE,
                    detailed:              detailed              => SignalFlags::DETAILED,
                    action:                action                => SignalFlags::ACTION,
                    no_hooks:              no_hooks              => SignalFlags::NO_HOOKS,
                    must_collect:          must_collect          => SignalFlags::MUST_COLLECT,
                    deprecated:            deprecated            => SignalFlags::DEPRECATED,
                    accumulator_first_run: accumulator_first_run => SignalFlags::ACCUMULATOR_FIRST_RUN
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
    pub emit: bool,
    pub connect: bool,
    pub handler: Option<syn::ImplItemMethod>,
    pub accumulator: Option<syn::ImplItemMethod>,
}

impl Signal {
    pub fn new(ident: syn::Ident) -> Self {
        Self {
            ident,
            name: Default::default(),
            flags: SignalFlags::empty(),
            interface: false,
            emit: true,
            connect: true,
            handler: None,
            accumulator: None,
        }
    }
    fn inputs(&self) -> impl Iterator<Item = &syn::FnArg> + Clone {
        self.handler
            .as_ref()
            .map(|s| s.sig.inputs.iter())
            .unwrap_or_else(|| {
                let acc = self.accumulator.as_ref().expect("no accumulator");
                abort!(acc, format!("No definition for signal `{}`", self.name));
            })
    }
    fn arg_names(&self) -> impl Iterator<Item = syn::Ident> + Clone + '_ {
        self.inputs()
            .enumerate()
            .map(|(i, _)| format_ident!("arg{}", i))
    }
    fn args_unwrap<'a>(
        &'a self,
        self_ty: Option<&'a syn::Type>,
        object_type: Option<&'a syn::Type>,
        imp: bool,
        glib: &'a TokenStream,
    ) -> impl Iterator<Item = TokenStream> + 'a {
        self.inputs().enumerate().map(move |(index, input)| {
            let ty = match input {
                syn::FnArg::Receiver(_) => {
                    let self_ty = if let Some(self_ty) = self_ty {
                        quote! { #self_ty }
                    } else {
                        quote! { Self }
                    };
                    if imp {
                        if let Some(ty) = object_type {
                            quote! { #ty }
                        } else {
                            quote! { <#self_ty as #glib::subclass::types::ObjectSubclass>::Type }
                        }
                    } else {
                        quote! { #self_ty }
                    }
                }
                syn::FnArg::Typed(t) => {
                    let ty = &t.ty;
                    quote! { #ty }
                }
            };
            let arg_name = format_ident!("arg{}", index);
            let unwrap_recv = match input {
                syn::FnArg::Receiver(_) => Some(quote! {
                    let #arg_name = #glib::subclass::prelude::ObjectSubclassIsExt::imp(&#arg_name);
                }),
                _ => None,
            };
            let err_msg = format!("Wrong type for argument {}: {{:?}}", index);
            quote! {
                let #arg_name = args[#index].get::<#ty>().unwrap_or_else(|e| {
                    panic!(#err_msg, e)
                });
                #unwrap_recv
            }
        })
    }
    pub fn create(
        &self,
        self_ty: &syn::Type,
        object_type: Option<&syn::Type>,
        glib: &TokenStream,
    ) -> TokenStream {
        let Self {
            name,
            flags,
            handler,
            accumulator,
            ..
        } = self;

        let handler = handler.as_ref().unwrap();
        let inputs = self.inputs();
        let input_static_types = inputs.skip(1).map(|input| {
            let ty = match &input {
                syn::FnArg::Typed(t) => &t.ty,
                _ => unimplemented!(),
            };
            quote! {
                <#glib::subclass::SignalType as ::core::convert::From<#glib::Type>>::from(
                    <#ty as #glib::types::StaticType>::static_type()
                )
            }
        });
        let arg_names = self.arg_names();
        let args_unwrap = self.args_unwrap(Some(self_ty), object_type, true, glib);
        let class_handler = (!handler.block.stmts.is_empty()).then(|| {
            let method_name = &handler.sig.ident;
            quote! {
                let builder = builder.class_handler(|_, args| {
                    #(#args_unwrap)*
                    let ret = #self_ty::#method_name(#(#arg_names),*);
                    #glib::closure::ToClosureReturnValue::to_closure_return_value(&ret)
                });
            }
        });
        let accumulator = accumulator.as_ref().map(|method| {
            let ident = &method.sig.ident;
            quote! {
                let builder = builder.accumulator(|hint, acc, value| {
                    #method
                    #ident(hint, acc, value)
                });
            }
        });
        let flags = (!flags.is_empty()).then(|| {
            let flags = flags.tokens(glib);
            quote! { let builder = builder.flags(#flags); }
        });
        let output = match &handler.sig.output {
            o @ syn::ReturnType::Type(_, _) => quote! { #o },
            _ => quote! { () },
        };
        quote! {
            {
                let param_types = [#(#input_static_types),*];
                let builder = #glib::subclass::Signal::builder(
                    #name,
                    &param_types,
                    <#glib::subclass::SignalType as ::core::convert::From<#glib::Type>>::from(
                        <#output as #glib::types::StaticType>::static_type()
                    ),
                );
                #flags
                #class_handler
                #accumulator
                builder.build()
            }
        }
    }
    pub fn handler_definition(&self) -> Option<TokenStream> {
        let handler = self.handler.as_ref().unwrap();
        if !handler.block.stmts.is_empty() {
            Some(quote! {
                #handler
            })
        } else {
            None
        }
    }
    fn emit_arg_defs(&self) -> impl Iterator<Item = syn::PatType> + Clone + '_ {
        self.inputs().skip(1).enumerate().map(|(index, arg)| {
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
            fn #method_name() -> &'static #glib::subclass::Signal
        }
    }
    pub fn signal_definition(
        &self,
        index: usize,
        signals_path: &TokenStream,
        glib: &TokenStream,
    ) -> TokenStream {
        let proto = self.signal_prototype(glib);
        quote! {
            #proto {
                &#signals_path()[#index]
            }
        }
    }
    pub fn emit_prototype(&self, glib: &TokenStream) -> TokenStream {
        let handler = self.handler.as_ref().unwrap();
        let output = &handler.sig.output;
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
        signals_path: &TokenStream,
        glib: &TokenStream,
    ) -> TokenStream {
        let proto = self.emit_prototype(glib);
        let handler = self.handler.as_ref().unwrap();
        let arg_defs = self.emit_arg_defs();
        let arg_names = arg_defs.clone().map(|arg| match &*arg.pat {
            syn::Pat::Ident(syn::PatIdent { ident, .. }) => ident.clone(),
            _ => unimplemented!(),
        });
        let signal_id = quote! { #signals_path()[#index].signal_id() };
        let emit = {
            let arg_names = arg_names.clone();
            quote! {
                <Self as #glib::object::ObjectExt>::emit(
                    self,
                    #signal_id,
                    &[#(&#arg_names),*]
                )
            }
        };
        let body = if self.flags.contains(SignalFlags::DETAILED) {
            quote! {
                if let Some(signal_details) = signal_details {
                    <Self as #glib::object::ObjectExt>::emit_with_details(
                        self,
                        #signal_id,
                        signal_details,
                        &[#(&#arg_names),*]
                    )
                } else {
                    #emit
                }
            }
        } else {
            emit
        };
        let unwrap = match &handler.sig.output {
            syn::ReturnType::Type(_, _) => Some(quote! {
                let ret = #glib::closure::TryFromClosureReturnValue::try_from_closure_return_value(
                    ret
                ).unwrap();
            }),
            _ => None,
        };
        quote! {
            #proto {
                let ret = #body;
                #unwrap
                ret
            }
        }
    }
    pub fn connect_prototype(&self, glib: &TokenStream) -> TokenStream {
        let method_name = format_ident!("connect_{}", self.name.to_snake_case());
        let handler = self.handler.as_ref().unwrap();
        let output = &handler.sig.output;
        let input_types = self.inputs().skip(1).map(|arg| match arg {
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
        signals_path: &TokenStream,
        glib: &TokenStream,
    ) -> TokenStream {
        let proto = self.connect_prototype(glib);
        let handler = self.handler.as_ref().unwrap();
        let arg_names = self.arg_names().skip(1);
        let args_unwrap = self.args_unwrap(None, None, false, glib).skip(1);

        let details = if self.flags.contains(SignalFlags::DETAILED) {
            quote! { details, }
        } else {
            quote! { ::std::option::Option::None }
        };

        let unwrap = match &handler.sig.output {
            syn::ReturnType::Type(_, _) => quote! {
                #glib::closure::ToClosureReturnValue::to_closure_return_value(&ret)
            },
            _ => quote! { ::core::option::Option::None },
        };
        quote! {
            #proto {
                <Self as #glib::object::ObjectExt>::connect_local_id(
                    self,
                    #signals_path()[#index].signal_id(),
                    #details,
                    false,
                    move |args| {
                        let recv = args[0].get::<Self>().unwrap();
                        #(#args_unwrap)*
                        let ret = f(&recv, #(#arg_names),*);
                        #unwrap
                    },
                )
            }
        }
    }
}
