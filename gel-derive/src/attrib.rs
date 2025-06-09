use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;

enum FieldAttr {
    Json,
    Rename(syn::LitStr),
}

enum ContainerAttr {
    Json,
    CratePath(syn::Path),
}

struct FieldAttrList(pub Punctuated<FieldAttr, syn::Token![,]>);
struct ContainerAttrList(pub Punctuated<ContainerAttr, syn::Token![,]>);

pub struct FieldAttrs {
    pub json: bool,
    pub rename: Option<syn::LitStr>,
}

pub struct ContainerAttrs {
    pub json: bool,
    pub crate_path: Option<syn::Path>,
}

impl ContainerAttrs {
    pub fn gel_protocol_path(&self) -> syn::Path {
        self.crate_path
            .clone()
            .unwrap_or(syn::parse_str("::gel_protocol").unwrap())
    }
}

mod kw {
    syn::custom_keyword!(json);
    syn::custom_keyword!(crate_path);
    syn::custom_keyword!(rename);
}

impl Parse for FieldAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(kw::json) {
            let _ident: syn::Ident = input.parse()?;
            Ok(FieldAttr::Json)
        } else if lookahead.peek(kw::rename) {
            input.parse::<kw::rename>()?;
            input.parse::<syn::Token![=]>()?;
            Ok(FieldAttr::Rename(input.parse()?))
        } else {
            Err(lookahead.error())
        }
    }
}

impl Parse for ContainerAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(kw::json) {
            let _ident: syn::Ident = input.parse()?;
            Ok(ContainerAttr::Json)
        } else if lookahead.peek(kw::crate_path) {
            input.parse::<kw::crate_path>()?;
            input.parse::<syn::Token![=]>()?;
            Ok(ContainerAttr::CratePath(input.parse()?))
        } else {
            Err(lookahead.error())
        }
    }
}

impl Parse for ContainerAttrList {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Punctuated::parse_terminated(input).map(ContainerAttrList)
    }
}

impl Parse for FieldAttrList {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Punctuated::parse_terminated(input).map(FieldAttrList)
    }
}

impl FieldAttrs {
    fn default() -> FieldAttrs {
        FieldAttrs {
            json: false,
            rename: None,
        }
    }
    pub fn from_syn(attrs: &[syn::Attribute]) -> syn::Result<FieldAttrs> {
        let mut res = FieldAttrs::default();
        for attr in attrs {
            if matches!(attr.style, syn::AttrStyle::Outer) && attr.path().is_ident("gel") {
                let chunk: FieldAttrList = attr.parse_args()?;
                for item in chunk.0 {
                    match item {
                        FieldAttr::Json => res.json = true,
                        FieldAttr::Rename(name) => {
                            if res.rename.is_some() {
                                return Err(syn::Error::new_spanned(
                                    name,
                                    "duplicate gel attribute `rename`",
                                ));
                            }
                            res.rename = Some(name)
                        }
                    }
                }
            }
        }
        Ok(res)
    }
}

impl ContainerAttrs {
    fn default() -> ContainerAttrs {
        ContainerAttrs {
            json: false,
            crate_path: None,
        }
    }
    pub fn from_syn(attrs: &[syn::Attribute]) -> syn::Result<ContainerAttrs> {
        let mut res = ContainerAttrs::default();
        for attr in attrs {
            if matches!(attr.style, syn::AttrStyle::Outer) && attr.path().is_ident("gel") {
                let chunk: ContainerAttrList = attr.parse_args()?;
                for item in chunk.0 {
                    match item {
                        ContainerAttr::Json => res.json = true,
                        ContainerAttr::CratePath(path) => {
                            if res.crate_path.is_some() {
                                return Err(syn::Error::new_spanned(
                                    path,
                                    "duplicate gel attribute `crate_path`",
                                ));
                            }
                            res.crate_path = Some(path)
                        }
                    }
                }
            }
        }
        Ok(res)
    }
}
