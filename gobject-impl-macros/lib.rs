#![feature(proc_macro_span)]

use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;

mod interface_impl;
mod object_impl;
mod property;
mod signal;
mod util;

#[proc_macro_attribute]
#[proc_macro_error]
pub fn object_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as object_impl::ObjectImplArgs);
    let item = syn::parse_macro_input!(item as syn::ItemImpl);
    object_impl::object_impl(args, item).into()
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn interface_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as interface_impl::InterfaceImplArgs);
    let item = syn::parse_macro_input!(item as syn::ItemImpl);
    interface_impl::interface_impl(args, item).into()
}
