#[derive(Debug)]
pub struct Root {
    pub objects: Vec<Object>,
}

#[derive(Debug)]
pub struct Object {
    pub name: Path,
}

#[derive(Debug)]
pub enum ObjectKind {
    Type(Type),
    Property(Property),
    Link,
}

#[derive(Debug)]
pub struct Type {}

#[derive(Debug)]
pub struct Property {}

#[derive(Debug)]
pub struct Path(pub Vec<String>);
