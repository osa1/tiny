mod syntax;
mod tree;

use std::collections::HashSet;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::Ident;
use syntax::Input;

use crate::syntax::Rule;

#[proc_macro]
pub fn byte_seq_parser(input: TokenStream) -> TokenStream {
    let Input {
        fn_name,
        fn_return_type,
        rules,
    } = syn::parse_macro_input!(input as syntax::Input);

    let is_valid_key = build_is_valid_key(&fn_name, &rules);

    let fn_body = tree::build_decision_tree(rules);
    let byte_parser = quote!(fn #fn_name(buf: &[u8]) -> Option<(#fn_return_type, usize)> {
        #fn_body
    });

    quote!(#is_valid_key #byte_parser).into()
}

fn build_is_valid_key(fn_name: &Ident, rules: &[Rule]) -> TokenStream2 {
    let keys = rules
        .iter()
        .map(|r| &r.value.0)
        .collect::<HashSet<_>>()
        .into_iter();
    let fn_name = format_ident!("{}_is_valid_key", fn_name);

    quote!(
        fn #fn_name(k: Key) -> bool {
            match k {
                #(#keys => true,)*
                _ => false,
            }
        }
    )
}
