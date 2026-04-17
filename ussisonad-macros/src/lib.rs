use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

/// # Panics
///
/// Panics if the annotated type is not a struct with named fields.
#[proc_macro_derive(UssisonadValue)]
pub fn derive_ussisonad_value(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            _ => panic!("UssisonadValue only supports named fields"),
        },
        _ => panic!("UssisonadValue only supports structs"),
    };

    let insertions = fields.iter().map(|f| {
        let field_name = f.ident.as_ref().unwrap();
        let key = field_name.to_string();
        quote! {
            map.insert(#key.into(), ::ussisonad_core::Value::from(self.#field_name));
        }
    });

    quote! {
        impl From<#name> for ::ussisonad_core::Value {
            fn from(self) -> ::ussisonad_core::Value {
                let mut map = ::std::collections::HashMap::new();
                #(#insertions)*
                ::ussisonad_core::Value::Object(map)
            }
        }
    }
    .into()
}
