use proc_macro::TokenStream;
use proc_macro2::Literal;
use quote::quote;
use syn::{
    parse_macro_input, punctuated::Punctuated, Data, DeriveInput, Expr, ExprLit, Fields, Ident,
    Lit, Token, Variant,
};

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
/// For enum types:
/// - The enum must have a `#[repr(type)]` attribute.
/// - The enum must have explicit discriminant values.
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
    let enum_name_str = enum_name.to_string();

    // Find the repr attribute to get the underlying type
    let repr_type = find_repr_type(&input.attrs)?;

    // Extract variants with their values
    let variants = extract_variants(&data.variants)?;

    // Separate variant names and values for iteration
    let variant_names: Vec<_> = variants.iter().map(|(name, _)| name).collect();
    let variant_values: Vec<_> = variants
        .iter()
        .map(|(_, value)| Literal::u64_unsuffixed(*value))
        .collect();

    // Generate the expanded code
    let expanded = quote! {
        impl #enum_name {
            const VALUES: &'static [(&'static str, usize)] = &[
                #(
                    (stringify!(#variant_names), #variant_values as _),
                )*
            ];
        }

        impl ::gel_protogen::prelude::EnumMeta for #enum_name {
            const VALUES: &'static [(&'static str, usize)] = Self::VALUES;
        }

        ::gel_protogen::prelude::declare_type!(::gel_protogen::prelude::DataType, #enum_name, flags=[enum], {});

        impl<'a> ::gel_protogen::prelude::DecoderFor<'a, #enum_name> for #enum_name {
            fn decode_for(buf: &mut &'a [u8]) -> Result<Self, ::gel_protogen::prelude::ParseError> {
                let repr = <#repr_type as ::gel_protogen::prelude::DecoderFor<#repr_type>>::decode_for(buf)?;

                match repr {
                    #(
                        #variant_values => Ok(#enum_name::#variant_names),
                    )*
                    _ => Err(ParseError::InvalidData(#enum_name_str, repr as usize)),
                }
            }
        }

        impl ::gel_protogen::prelude::EncoderFor<#enum_name> for #enum_name {
            fn encode_for(&self, buf: &mut ::gel_protogen::prelude::BufWriter<'_>) {
                <#repr_type as ::gel_protogen::prelude::EncoderFor<#repr_type>>::encode_for(&(*self as #repr_type), buf);
            }
        }

        impl ::gel_protogen::prelude::EncoderFor<#enum_name> for &'_ #enum_name {
            fn encode_for(&self, buf: &mut ::gel_protogen::prelude::BufWriter<'_>) {
                <#repr_type as ::gel_protogen::prelude::EncoderFor<#repr_type>>::encode_for(&(**self as #repr_type), buf);
            }
        }

        impl ::std::convert::TryFrom<#repr_type> for #enum_name {
            type Error = ::gel_protogen::prelude::ParseError;
            fn try_from(value: #repr_type) -> Result<Self, Self::Error> {
                Ok(match value {
                    #(
                        #variant_values => #enum_name::#variant_names,
                    )*
                    _ => return Err(::gel_protogen::prelude::ParseError::InvalidData(#enum_name_str, value as usize)),
                })
            }
        }

        impl ::std::convert::From<#enum_name> for #repr_type {
            fn from(value: #enum_name) -> Self {
                match value {
                    #(
                        #enum_name::#variant_names => #enum_name::#variant_names as _,
                    )*
                }
            }
        }
    };

    Ok(expanded)
}

fn find_repr_type(attrs: &[syn::Attribute]) -> syn::Result<syn::Type> {
    for attr in attrs {
        if attr.path().is_ident("repr") {
            let tokens = attr.meta.require_list()?.parse_args_with(
                syn::punctuated::Punctuated::<syn::Type, Token![,]>::parse_terminated,
            )?;
            if let Some(repr_type) = tokens.into_iter().next() {
                return Ok(repr_type);
            }
        }
    }

    Err(syn::Error::new_spanned(
        &attrs[0],
        "Protocol enum must have a #[repr(type)] attribute",
    ))
}

fn extract_variants(variants: &Punctuated<Variant, Token![,]>) -> syn::Result<Vec<(Ident, u64)>> {
    let mut result = Vec::new();

    for variant in variants {
        // Check that the variant has unit fields
        match &variant.fields {
            Fields::Unit => {}
            _ => {
                return Err(syn::Error::new_spanned(
                    &variant.ident,
                    "Protocol enum variants must have unit fields",
                ))
            }
        }

        // Extract the discriminant value
        let value = if let Some((_, discriminant)) = &variant.discriminant {
            extract_literal_value(discriminant)?
        } else {
            return Err(syn::Error::new_spanned(
                &variant.ident,
                "Protocol enum variants must have explicit discriminant values",
            ));
        };

        result.push((variant.ident.clone(), value));
    }

    Ok(result)
}

fn extract_literal_value(expr: &Expr) -> syn::Result<u64> {
    match expr {
        Expr::Lit(ExprLit { lit: Lit::Int(lit_int), .. }) => {
            lit_int.base10_parse::<u64>()
        },
        Expr::Lit(ExprLit { lit: Lit::Char(lit_char), .. }) => {
            Ok(lit_char.value() as u64)
        },
        Expr::Lit(ExprLit { lit: Lit::Byte(lit_byte), .. }) => {
            Ok(lit_byte.value() as u64)
        },
        _ => Err(syn::Error::new_spanned(
            expr,
            "Protocol enum variant discriminant must be a literal integer or character (eg: 1, 'A', b'A')"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::TokenStream;

    #[test]
    fn test_protocol_derive() {
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
    }
}
