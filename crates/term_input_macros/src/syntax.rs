use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;

#[derive(Debug)]
pub(crate) struct Pattern(pub(crate) Vec<u8>);

#[derive(Debug)]
pub(crate) struct Value(pub(crate) syn::Expr);

#[derive(Debug)]
pub(crate) struct Rule {
    pub(crate) pattern: Pattern,
    pub(crate) value: Value,
}

#[derive(Debug)]
pub(crate) struct Input {
    pub(crate) fn_name: syn::Ident,
    pub(crate) fn_return_type: syn::Type,
    pub(crate) rules: Vec<Rule>,
}

fn parse_byte_array_elems(array: syn::ExprArray) -> syn::Result<Vec<u8>> {
    let mut ret = Vec::with_capacity(array.elems.len());

    if array.elems.is_empty() {
        return Err(syn::Error::new(array.span(), "blah"));
    }

    for pair in array.elems.into_pairs() {
        let lit = match pair.into_value() {
            syn::Expr::Lit(lit) => lit,
            other => {
                return Err(syn::Error::new(other.span(), "blah"));
            }
        };

        match lit.lit {
            syn::Lit::Byte(byte) => ret.push(byte.value()),
            syn::Lit::Int(int) => {
                let val = int.base10_parse::<u8>()?;
                ret.push(val);
            }
            other => {
                return Err(syn::Error::new(other.span(), "blah"));
            }
        }
    }

    Ok(ret)
}

fn parse_byte_string(str: syn::LitByteStr) -> syn::Result<Vec<u8>> {
    if str.value().is_empty() {
        return Err(syn::Error::new(str.span(), "blah"));
    }

    Ok(str.value())
}

impl Parse for Pattern {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Either a u8 array with just literals, or a byte string
        let string = input.parse::<syn::LitByteStr>();
        let array = input.parse::<syn::ExprArray>();

        match array {
            Ok(array) => Ok(Pattern(parse_byte_array_elems(array)?)),
            Err(_) => match string {
                Err(_) => Err(syn::Error::new(input.span(), "blah")),
                Ok(str) => Ok(Pattern(parse_byte_string(str)?)),
            },
        }
    }
}

impl Parse for Value {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Value(input.parse()?))
    }
}

impl Parse for Rule {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let pattern = input.parse()?;
        input.parse::<syn::Token![=>]>()?;
        let value = input.parse()?;
        Ok(Rule { pattern, value })
    }
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let fn_name = input.parse::<syn::Ident>()?;
        input.parse::<syn::Token![->]>()?;
        let fn_return_type = input.parse::<syn::Type>()?;

        input.parse::<syn::Token![,]>()?;
        let rules = syn::punctuated::Punctuated::<Rule, syn::Token![,]>::parse_terminated(input)?
            .into_pairs()
            .map(|pair| pair.into_value())
            .collect();
        Ok(Input {
            fn_name,
            fn_return_type,
            rules,
        })
    }
}
