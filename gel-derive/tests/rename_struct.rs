use gel_derive::Queryable;

#[derive(Queryable)]
#[allow(dead_code)]
struct Simple {
    #[gel(rename = "final")]
    r#final: String,
    #[gel(rename = "self")]
    self_: i16,
}
