use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;

mod interface_impl;
mod object_impl;
mod property;
mod signal;
mod util;
use util::*;

#[proc_macro_attribute]
#[proc_macro_error]
pub fn object_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as Args);
    object_impl::object_impl(args, item).into()
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn interface_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as Args);
    interface_impl::interface_impl(args, item).into()
}
