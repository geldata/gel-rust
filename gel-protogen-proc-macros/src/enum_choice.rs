use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{punctuated::Punctuated, Token, Variant};

pub struct EnumChoiceVariant {
    pub name: Ident,
    pub message_type: syn::Type,
}

pub fn extract_variants(
    variants: &Punctuated<Variant, Token![,]>,
) -> syn::Result<Vec<EnumChoiceVariant>> {
    let mut result = Vec::new();
    for variant in variants {
        let variant_name = &variant.ident;
        let message_type = match &variant.fields {
            syn::Fields::Unnamed(fields) => {
                if fields.unnamed.len() == 1 {
                    &fields.unnamed[0].ty
                } else {
                    return Err(syn::Error::new_spanned(
                        variant,
                        "Enum choice variants must be tuple variants with one field",
                    ));
                }
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    variant,
                    "Enum choice variants must be tuple variants with one field",
                ));
            }
        };
        result.push(EnumChoiceVariant {
            name: variant_name.clone(),
            message_type: message_type.clone(),
        });
    }
    Ok(result)
}

pub fn generate_enum_choice_from_variants(
    enum_name: &Ident,
    variants: &Vec<EnumChoiceVariant>,
) -> syn::Result<TokenStream> {
    let variant_names: Vec<_> = variants
        .iter()
        .map(|variant| variant.name.clone())
        .collect();
    let variant_message_types: Vec<_> = variants
        .iter()
        .map(|variant| variant.message_type.clone())
        .collect();

    Ok(quote! {
        impl<'a> ::gel_protogen::prelude::DataType for #enum_name<'a> {
            const META: ::gel_protogen::prelude::StructFieldMeta =
                ::gel_protogen::prelude::StructFieldMeta::new(stringify!(#enum_name), None);
        }

        impl<'a> ::gel_protogen::prelude::DecoderFor<'a, #enum_name<'a>> for #enum_name<'a> {
            fn decode_for(buf: &mut &'a [u8]) -> Result<Self, ::gel_protogen::prelude::ParseError> {
                #(
                    if <#variant_message_types>::is_buffer(buf) {
                        Ok(Self::#variant_names(<#variant_message_types as ::gel_protogen::prelude::DecoderFor<#variant_message_types>>::decode_for(buf)?))
                    } else
                )*
                {
                    Err(::gel_protogen::prelude::ParseError::InvalidData(stringify!(#enum_name), 0))
                }
            }
        }

        impl<'a> ::gel_protogen::prelude::EncoderFor<#enum_name<'static>> for #enum_name<'a> {
            fn encode_for(&self, buf: &mut ::gel_protogen::prelude::BufWriter<'_>) {
                match self {
                    #(
                        Self::#variant_names(message) => <
                            #variant_message_types as ::gel_protogen::prelude::EncoderFor<::gel_protogen::prelude::make_static!(#variant_message_types)>
                        >::encode_for(message, buf),
                    )*
                }
            }
        }
    })
}
