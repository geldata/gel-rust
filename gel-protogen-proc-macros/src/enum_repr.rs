use proc_macro2::Literal;
use quote::quote;
use syn::{
    punctuated::Punctuated, Expr, ExprLit, Fields, Ident, Lit, Token, Variant,
};

/// Extract the repr type from enum attributes
pub fn find_repr_type(attrs: &[syn::Attribute]) -> syn::Result<syn::Type> {
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

/// Extract variants with their discriminant values from an enum
pub fn extract_variants(variants: &Punctuated<Variant, Token![,]>) -> syn::Result<Vec<(Ident, u64)>> {
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

/// Extract literal value from an expression (integer, char, or byte)
pub fn extract_literal_value(expr: &Expr) -> syn::Result<u64> {
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

/// Generate the basic enum implementation code
pub fn generate_enum_impl(
    enum_name: &Ident,
    enum_name_str: &str,
    repr_type: &syn::Type,
    variants: &[(Ident, u64)],
) -> proc_macro2::TokenStream {
    // Separate variant names and values for iteration
    let variant_names: Vec<_> = variants.iter().map(|(name, _)| name).collect();
    let variant_values: Vec<_> = variants
        .iter()
        .map(|(_, value)| Literal::u64_unsuffixed(*value))
        .collect();

    quote! {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::TokenStream;
    use syn::{Data, DeriveInput};
    
    #[test]
    fn test_extract_variants() {
        let input: TokenStream = quote! {
            #[repr(u8)]
            enum TestEnum {
                A = 1,
                B = 2,
                C = b'A',
            }
        };

        let input: DeriveInput = syn::parse2(input).unwrap();
        if let Data::Enum(data_enum) = input.data {
            let result = extract_variants(&data_enum.variants);
            assert!(result.is_ok());
            
            let variants = result.unwrap();
            assert_eq!(variants.len(), 3);
            assert_eq!(variants[0].0.to_string(), "A");
            assert_eq!(variants[0].1, 1);
            assert_eq!(variants[1].0.to_string(), "B");
            assert_eq!(variants[1].1, 2);
            assert_eq!(variants[2].0.to_string(), "C");
            assert_eq!(variants[2].1, 65); // b'A' = 65
        }
    }

    #[test]
    fn test_find_repr_type() {
        let input: TokenStream = quote! {
            #[repr(u8)]
            enum TestEnum {
                A = 1,
            }
        };

        let input: DeriveInput = syn::parse2(input).unwrap();
        let result = find_repr_type(&input.attrs);
        assert!(result.is_ok());
        
        let repr_type = result.unwrap();
        assert_eq!(quote!(#repr_type).to_string(), "u8");
    }
} 