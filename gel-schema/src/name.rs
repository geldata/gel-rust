use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::Class;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Name {
    pub module: Option<String>,
    pub object: String,
}

impl Name {
    pub fn new_unqualified(object: String) -> Self {
        Name {
            module: None,
            object,
        }
    }

    pub fn new_from_string<'a>(s: impl Into<Cow<'a, str>>) -> Self {
        let s = s.into();
        if let Some((module, object)) = s.rsplit_once("::") {
            Name {
                module: Some(module.to_string()),
                object: object.to_string(),
            }
        } else {
            Name::new_unqualified(s.into_owned())
        }
    }

    pub fn into_unqualified(mut self) -> Name {
        self.module = None;
        self
    }

    /// Analogous to Object.get_shortname_static
    pub fn as_shortname(&self, cls: Class) -> Name {
        match cls {
            Class::Parameter => Name {
                module: Some("__".into()),
                object: self.clone().fullname_into_paramname(),
            },
            Class::Index => self.clone().fullname_into_shortname(),

            Class::ExtensionPackage | Class::ExtensionPackageMigration => {
                Name::new_unqualified(self.clone().fullname_into_shortname().object)
            }

            c if c.is_qualified() => {
                // ported from QualifiedObject.get_shortname_static
                let result = self.clone().fullname_into_shortname();
                if result.module.is_none() {
                    Name {
                        module: self.module.clone(),
                        object: result.object,
                    }
                } else {
                    result
                }
            }

            _ => self.clone(),
        }
    }

    /// Analogous to paramname_from_fullname
    fn fullname_into_paramname(self) -> String {
        if let Some((first, _second)) = self.object.split_once('@') {
            unmangle_name(first)
        } else {
            self.object.clone()
        }
    }

    /// Analogous to shortname_from_fullname
    pub fn fullname_into_shortname(self) -> Name {
        if let Some((first, _second)) = self.object.split_once('@') {
            Self::new_from_string(unmangle_name(first))
        } else {
            self
        }
    }
}

#[allow(dead_code)]
pub fn mangle_name(name: &str) -> String {
    name.replace('|', "||")
        .replace('&', "&&")
        .replace("::", "|")
        .replace('@', "&")
}

static MANGLE_RE_1: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([^|])\|([^|])").unwrap());
static MANGLE_RE_2: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([^&])&([^&])").unwrap());

pub fn unmangle_name(name: &str) -> String {
    let name = MANGLE_RE_1.replace_all(name, "$1::$2");
    let name = MANGLE_RE_2.replace_all(&name, "$1@$2");
    name.replace("||", "|").replace("&&", "&")
}

impl std::fmt::Debug for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(module) = &self.module {
            f.write_str(module)?;
            f.write_str("::")?;
        }
        f.write_str(&self.object)
    }
}

#[test]
fn mangle_00() {
    let name = Name {
        module: Some("std::net::http".into()),
        object: "schedule_request".into(),
    };

    assert_eq!(
        name.fullname_into_shortname(),
        Name {
            module: Some("std::net::http".into()),
            object: "schedule_request".into()
        }
    )
}
