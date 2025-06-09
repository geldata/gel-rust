// In the gel-derive crate we want to extend the `Queryable` macro to support the following.
// A struct and enum attribute (`ContainerAttr`) called crate_path which customises the crate path for the `gel_protocol`.
// Currently it is hardcoded to `::gel_protocol` but should support adding custom values like `::gelx::exports::gel_protocol`.
// The second thing is to add a new field attribute called `rename` which allows renaming the field for both enums and structs.
// After these changes are made the following should be possible.

use gel_derive::Queryable;

mod custom {
    pub mod exports {
        pub mod gel_protocol {
            pub use ::gel_protocol::*;
        }
    }
}

#[derive(Queryable)]
#[gel(crate_path = custom::exports::gel_protocol)]
enum Simple {
    #[gel(rename = "something")]
    Something,
    #[gel(rename = "else")]
    Else,
}

#[derive(Queryable)]
#[gel(crate_path = custom::exports::gel_protocol)]
struct Another {
    #[gel(rename = "final")]
    pub r#final: String,
    #[gel(rename = "self")]
    pub self_: i16,
}
