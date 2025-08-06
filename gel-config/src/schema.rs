use indexmap::IndexMap;
use crate::PrimitiveType;

pub fn primitive(typ: PrimitiveType) -> Type {
    Type::Primitive(typ)
}

pub fn enumeration<I, S>(name: impl ToString, choices: I) -> Type
where
    S: ToString,
    I: IntoIterator<Item = S>,
{
    Type::Enum {
        name: name.to_string(),
        choices: choices.into_iter().map(|s| s.to_string()).collect(),
    }
}

pub struct Schema(pub Vec<ModuleSchema>);

pub struct ModuleSchema {
    pub extension_name: Option<String>,
    pub object_types: IndexMap<String, ObjectType>,
}

#[derive(Clone)]
pub struct ObjectType {
    /// Pointer of the object (properties and links)
    pub pointers: IndexMap<String, Pointer>,

    /// When a type is top-level, it's properties are configured with `configure set {obj}::{prop} := ...`
    /// Otherwise, it is configured with `configure insert {obj} := { {prop} := ... };`
    pub is_top_level: bool,

    /// Indicates that this object cannot be identified just by its name. This happens, for example,
    /// when it is used as a link on a multi object. `configure insert {obj}` does not indicate which parent
    /// object this object belongs to. Instead these objects are inserted in a nested insert stmt.
    pub is_non_locatable: bool,

    /// Indicates that the object is used as multi property, so we should expect arrays instead of tables.
    pub is_multi: bool,
}

#[derive(Debug, Clone)]
pub struct Pointer {
    pub target: Type,

    pub is_required: bool,
    pub is_multi: bool,

    pub description: Option<String>,
    pub deprecated: Option<String>,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Type {
    Primitive(PrimitiveType),
    Enum { name: String, choices: Vec<String> },
    ObjectRef(String),

    // TODO: this is not really a union, it should be an abstract type and sub types
    Union(Vec<String>),
}

impl Schema {
    pub fn find_object(&self, key: &str) -> Option<(Option<&str>, &ObjectType)> {
        for s in &self.0 {
            if let Some(obj) = s.find_object(key) {
                return Some((s.extension_name.as_deref(), obj));
            }
        }
        None
    }

    pub fn std(&mut self) -> &mut ModuleSchema {
        self.push(None)
    }

    pub fn ext(&mut self, extension_name: impl ToString) -> &mut ModuleSchema {
        self.push(Some(extension_name.to_string()))
    }

    fn push(&mut self, extension_name: Option<String>) -> &mut ModuleSchema {
        let schema = ModuleSchema {
            extension_name,
            object_types: Default::default(),
        };
        self.0.push(schema);
        self.0.last_mut().unwrap()
    }
}

impl ModuleSchema {
    pub fn find_object<'s>(&'s self, key: &str) -> Option<&'s ObjectType> {
        self.object_types.get(key)
    }

    pub fn register(&mut self, name: impl ToString, obj: ObjectType) -> Type {
        for (_, ptr) in &obj.pointers {
            let mut child_links = Vec::new();

            for target_ref in ptr.target.get_object_refs() {
                let target = self.object_types.get_mut(target_ref).unwrap();
                target.is_top_level = false;
                target.is_multi = ptr.is_multi;

                for (_, ptr) in &target.pointers {
                    child_links.extend(ptr.target.get_object_refs().into_iter().cloned());
                }
            }

            if ptr.is_multi {
                // set is_non_locatable
                let mut descendant_links = child_links;
                while let Some(obj_ref) = descendant_links.pop() {
                    let obj = self.object_types.get_mut(&obj_ref).unwrap();
                    obj.is_non_locatable = true;

                    for (_, ptr) in &obj.pointers {
                        descendant_links.extend(ptr.target.get_object_refs().into_iter().cloned());
                    }
                }
            }
        }

        self.object_types.insert(name.to_string(), obj);
        Type::ObjectRef(name.to_string())
    }
}

impl ObjectType {
    pub fn new<I, S>(pointers: I) -> Self
    where
        S: ToString,
        I: IntoIterator<Item = (S, Pointer)>,
    {
        Self {
            pointers: pointers
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            is_top_level: true,
            is_multi: false,
            is_non_locatable: false,
        }
    }
}

impl Type {
    pub fn is_scalar(&self) -> bool {
        match self {
            Type::Primitive(_) => true,
            Type::Enum { .. } => true,
            Type::ObjectRef(_) => false,
            Type::Union(_) => false,
        }
    }

    pub fn get_object_refs(&self) -> Vec<&String> {
        match self {
            Type::Primitive(_) => Vec::new(),
            Type::Enum { .. } => Vec::new(),
            Type::ObjectRef(r) => vec![r],
            Type::Union(components) => components.iter().collect(),
        }
    }

    pub fn new_union(objects: impl IntoIterator<Item = Type>) -> Self {
        Type::Union(
            objects
                .into_iter()
                .map(|t| match t {
                    Type::ObjectRef(r) => r,
                    _ => panic!(),
                })
                .collect(),
        )
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self, f)
    }
}

impl Pointer {
    pub fn new(target: Type) -> Pointer {
        Pointer {
            target,
            is_multi: false,
            is_required: false,
            deprecated: None,
            description: None,
            examples: Vec::new(),
        }
    }

    pub fn multi(mut self) -> Pointer {
        self.is_multi = true;
        self
    }

    pub fn required(mut self) -> Pointer {
        self.is_required = true;
        self
    }
}

#[allow(dead_code)]
impl Pointer {
    fn with_description(mut self, description: impl ToString) -> Self {
        self.description = Some(description.to_string());
        self
    }

    fn with_deprecated(mut self, deprecated: impl ToString) -> Self {
        self.deprecated = Some(deprecated.to_string());
        self
    }

    fn with_examples<I, S>(mut self, examples: I) -> Self
    where
        S: ToString,
        I: IntoIterator<Item = S>,
    {
        self.examples = examples.into_iter().map(|s| s.to_string()).collect();
        self
    }
}

pub trait Optional {
    fn optional(self, key: &'static str) -> impl Iterator<Item = (String, Pointer)> + Clone;
}

impl<I> Optional for I
where
    I: Iterator<Item = (String, Pointer)> + Clone,
{
    fn optional(self, key: &'static str) -> impl Iterator<Item = (String, Pointer)> + Clone {
        self.map(move |(k, mut p)| {
            if k.as_str() == key {
                p.is_required = false;
            }
            (k, p)
        })
    }
}
