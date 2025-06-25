use proc_macro::TokenStream;
use syn::{parse_macro_input, Data, DeriveInput};

mod enum_choice;
mod enum_repr;

/// Derive macro for Protocol types
///
/// This macro generates the necessary implementations for protocol types,
/// including encoding/decoding, metadata, and conversion traits.
///
/// This macro auto-generates implementations for the following traits:
///
/// - `EnumMeta`
/// - `DataType`
/// - `DecoderFor`
/// - `EncoderFor`
/// - `TryFrom`
/// - `From`
///
/// # Requirements
///
/// For repr enum types:
/// - The enum must have a `#[repr(type)]` attribute.
/// - The enum must have explicit discriminant values.
/// - The enum must be `Copy`
///
/// For enum choice types:
/// - The enum must not have repr attributes.
/// - Each variant must be a struct with a `#[derive(Protocol)]` attribute.
/// - The enum must be `Copy`
///
/// # Example
///
/// ```nocompile
/// use gel_protogen_proc_macros::Protocol;
///
/// #[derive(Protocol)]
/// #[repr(u8)]
/// enum MyEnum {
///     A = 1,
///     B = 2,
/// }
///
/// #[derive(Protocol)]
/// enum MyEnum<'a> {
///     A(A<'a>),
///     B(B<'a>),
///     C(C<'a>),
/// }
/// ```
#[proc_macro_derive(Protocol)]
pub fn derive_protocol(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match derive_protocol_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn derive_protocol_impl(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    // Check if it's an enum
    let data = match input.data {
        Data::Enum(data_enum) => data_enum,
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "Protocol derive macro only supports enums for now",
            ))
        }
    };

    let enum_name = &input.ident;

    // Find the repr attribute to get the underlying type
    match enum_repr::find_repr_type(&input.attrs)? {
        Some(repr_type) => {
            let variants = enum_repr::extract_variants(&data.variants)?;
            let expanded = enum_repr::generate_enum_impl(enum_name, &repr_type, &variants);
            Ok(expanded)
        }
        None => {
            // This is an enum choice type - extract variants and generate choice implementation
            let variants = enum_choice::extract_variants(&data.variants)?;
            let expanded = enum_choice::generate_enum_choice_from_variants(enum_name, &variants)?;
            Ok(expanded)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::TokenStream;
    use quote::quote;

    #[test]
    fn test_enum_repr() {
        let input: TokenStream = quote! {
            #[repr(u8)]
            enum TestEnum {
                A = 1,
                B = 2,
                C = b'A',
            }
        };

        let input = syn::parse2(input).unwrap();
        let result = derive_protocol_impl(input);
        assert!(result.is_ok());
        eprintln!("{}", result.unwrap());
    }

    #[test]
    fn test_enum_choice() {
        let input: TokenStream = quote! {
            enum TestEnum<'a> {
                A(A<'a>),
                B(B<'a>),
                C(C<'a>),
            }
        };

        let input = syn::parse2(input).unwrap();
        let result = derive_protocol_impl(input);
        assert!(result.is_ok());
        eprintln!("{}", result.unwrap());
    }
}
