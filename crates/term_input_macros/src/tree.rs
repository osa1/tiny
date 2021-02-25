use crate::syntax::*;

use proc_macro2::TokenStream;
use quote::quote;
use quote::TokenStreamExt;
use std::collections::HashMap;

struct Node {
    idx: usize,
    value: Option<syn::Expr>,
    next: HashMap<u8, Node>,
}

impl Node {
    fn new(idx: usize) -> Self {
        Node {
            idx,
            value: None,
            next: HashMap::new(),
        }
    }

    fn to_token_stream(&self) -> TokenStream {
        let Node { idx, value, next } = self;

        let node_value = match value {
            None => quote!(None),
            Some(expr) => quote!(Some((#expr, #idx))),
        };

        // Optimize the case when the node is a leaf. Not necessary for correctness, but makes the
        // generated code smaller.
        if next.is_empty() {
            return quote!(#node_value);
        }

        let mut match_arms = vec![];
        for (byte, next) in next.iter() {
            let next_tokens = next.to_token_stream();
            match_arms.push(quote!(
                #byte => {
                    #next_tokens
                }
            ));
        }
        match_arms.push(quote!(_ => #node_value));

        quote!(
            match buf.get(#idx) {
                None => #node_value,
                Some(byte) => {
                    match byte {
                        #(#match_arms,)*
                    }
                }
            }
        )
    }

    fn add_rule(&mut self, rule: Rule) {
        let Rule { pattern, value } = rule;
        let pattern: Vec<u8> = pattern.0;
        let value: syn::Expr = value.0;

        let byte = pattern[0];
        let rest = &pattern[1..];

        match self.next.get_mut(&byte) {
            None => {
                let mut node = Node::new(1);
                node.add_rule_(rest, value);
                self.next.insert(byte, node);
            }
            Some(node) => {
                node.add_rule_(rest, value);
            }
        }
    }

    fn add_rule_(&mut self, bytes: &[u8], value: syn::Expr) {
        if bytes.is_empty() {
            assert!(self.value.is_none()); // TODO: improve the err msg
            self.value = Some(value);
        } else {
            let byte = bytes[0];
            let rest = &bytes[1..];

            match self.next.get_mut(&byte) {
                None => {
                    let mut node = Node::new(self.idx + 1);
                    node.add_rule_(rest, value);
                    self.next.insert(byte, node);
                }
                Some(node) => {
                    node.add_rule_(rest, value);
                }
            }
        }
    }
}

pub(crate) fn build_decision_tree(rules: Vec<Rule>) -> TokenStream {
    let mut stream = TokenStream::new();
    let mut tree = Node::new(0);
    for rule in rules {
        tree.add_rule(rule);
    }

    stream.append_all(tree.to_token_stream());

    stream
}
