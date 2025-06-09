use gel_derive::Queryable;

#[derive(Queryable)]
enum Simple {
    #[gel(rename = "something")]
    Something,
    #[gel(rename = "else")]
    Else,
}
