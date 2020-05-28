mod syntax;
mod tree;

use syntax::Input;

use proc_macro::TokenStream;
use quote::quote;

#[proc_macro]
pub fn byte_seq_parser(input: TokenStream) -> TokenStream {
    let Input {
        fn_name,
        fn_return_type,
        rules,
    } = syn::parse_macro_input!(input as syntax::Input);
    let fn_body = tree::build_decision_tree(rules);

    quote!(fn #fn_name(buf: &[u8]) -> Option<(#fn_return_type, usize)> {
        #fn_body
    })
    .into()
}
