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
    Something,
    Else,
}

#[derive(Queryable)]
#[gel(crate_path = custom::exports::gel_protocol)]
#[allow(dead_code)]
struct Another {
    one: String,
    two: i16,
}
