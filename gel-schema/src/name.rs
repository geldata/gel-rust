use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::Class;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

    /// Analogous to Object.get_shortname_static
    pub fn as_short_name(&self, cls: Class) -> Name {
        match cls {
            Class::Parameter => Name {
                module: Some("__".into()),
                object: self.clone().fullname_into_param_name(),
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
    fn fullname_into_param_name(self) -> String {
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

    /// Analogous to Object.get_displayname_static
    pub fn as_display_name(&self, cls: Class) -> String {
        match cls {
            _ if cls.is_subclass(&Class::Pointer)
                || cls.is_subclass(&Class::NamedReferencedInheritingObject) =>
            {
                let sn = self.as_short_name(cls);
                if sn.module.as_deref() == Some("__") {
                    sn.object
                } else {
                    sn.to_string()
                }
            }
            Class::Parameter | Class::ExtensionPackage | Class::ExtensionPackageMigration => {
                let sn = self.as_short_name(cls);
                sn.object
            }
            _ if cls.is_subclass(&Class::Collection) => {
                let name = self.to_string();

                if self.module.is_some() {
                    // FIXME: Globals and alias names do mangling but *don't*
                    // duplicate the module name, which most places do.
                    name.split_once('@')
                        .map(|(x, _)| x.to_string())
                        .unwrap_or(name)
                } else {
                    recursively_unmangle_shortname(&name).into_owned()
                }
            }
            _ => self.as_short_name(cls).to_string(),
        }
    }

    /// Analogous to Object.get_verbosename_static
    pub fn as_verbose_name(&self, cls: Class, parent: Option<&str>) -> String {
        let cls_name = cls.get_display_name();
        let dname = self.as_display_name(cls);
        if let Some(parent) = parent {
            format!("{cls_name} '{dname}' of {parent}")
        } else {
            format!("{cls_name} '{dname}'")
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

static UNMANGLE_RE_1: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\|+").unwrap());

/// Any number of pipes becomes a single ::.
pub fn recursively_unmangle_shortname(name: &str) -> Cow<str> {
    UNMANGLE_RE_1.replace_all(name, "::")
}

impl std::fmt::Display for Name {
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
