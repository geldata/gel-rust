use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Debug;
use std::str::FromStr;

pub struct SystemEnvVars;

/// A trait for reading environment variables. By default, uses `std::env::Vars`
/// but can be re-implemented for other sources.
pub trait EnvVar {
    fn default() -> impl EnvVar {
        SystemEnvVars
    }
    fn read(&self, name: &str) -> Result<Cow<str>, std::env::VarError>;
}

#[derive(Debug, derive_more::Display, derive_more::Error)]
#[display("Failed to parse environment variable {name}: {value}")]
pub struct EnvParseError {
    pub name: String,
    pub value: String,
    #[error(source)]
    source: EnvParseErrorSource,
}

#[derive(Debug, derive_more::Display, derive_more::Error)]
enum EnvParseErrorSource {
    None,
    Source(Box<dyn std::error::Error + Send + Sync>),
}

impl EnvParseError {
    pub fn new(name: String, value: String) -> Self {
        Self {
            name,
            value,
            source: EnvParseErrorSource::None,
        }
    }

    pub fn source(mut self, source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        self.source = EnvParseErrorSource::Source(source.into());
        self
    }
}

impl<K, V> EnvVar for HashMap<K, V>
where
    K: std::hash::Hash + Eq + std::borrow::Borrow<str>,
    V: std::borrow::Borrow<str>,
{
    fn read(&self, name: &str) -> Result<Cow<str>, std::env::VarError> {
        self.get(name)
            .map(|value| value.borrow().into())
            .ok_or(std::env::VarError::NotPresent)
    }
}

impl EnvVar for SystemEnvVars {
    fn read(&self, name: &str) -> Result<Cow<str>, std::env::VarError> {
        if let Ok(value) = std::env::var(name) {
            Ok(value.into())
        } else {
            Err(std::env::VarError::NotPresent)
        }
    }
}

impl EnvVar for &[(&str, &str)] {
    fn read(&self, name: &str) -> Result<Cow<str>, std::env::VarError> {
        for (key, value) in self.iter() {
            if *key == name {
                return Ok((*value).into());
            }
        }
        Err(std::env::VarError::NotPresent)
    }
}

impl EnvVar for () {
    fn read(&self, _: &str) -> Result<Cow<str>, std::env::VarError> {
        Err(std::env::VarError::NotPresent)
    }
}

impl<T> EnvVar for &T
where
    T: EnvVar,
{
    fn read(&self, name: &str) -> Result<Cow<str>, std::env::VarError> {
        (*self).read(name)
    }
}

pub use crate::__UNEXPORTED_define_env as define_env;
use crate::gel::{BuildContext, FromParamStr};

#[doc(hidden)]
#[macro_export]
macro_rules! __UNEXPORTED_define_env {
    (
        type Error = $error:ty;
        $(
            #[doc=$doc:expr]
            #[env($($env_name:expr),+)]
            $(#[preprocess=$preprocess:expr])?
            $(#[parse=$parse:expr])?
            $(#[validate=$validate:expr])?
            $name:ident: $type:ty
        ),* $(,)?
    ) => {
        #[derive(Debug, Clone)]
        pub struct Env {
        }

        #[allow(clippy::diverging_sub_expression)]
        impl Env {
            $(
                #[doc = $doc]
                pub fn $name(context: &mut impl $crate::gel::BuildContext) -> ::std::result::Result<::std::option::Option<$type>, $error> {
                    const ENV_NAMES: &[&str] = &[$(stringify!($env_name)),+];
                    let Some((_name, s)) = $crate::env::get_envs(ENV_NAMES, context)? else {
                        return Ok(None);
                    };
                    $(let Some(s) = $preprocess(&s, context)? else {
                        return Ok(None);
                    };)?

                    // This construct lets us choose between $parse and std::str::FromStr
                    // without requiring all types to implement FromStr.
                    #[allow(unused_labels)]
                    let value: $type = 'block: {
                        $(
                            break 'block $parse(&name, &s)?;

                            // Disable the fallback parser
                            #[cfg(all(debug_assertions, not(debug_assertions)))]
                        )?
                        $crate::env::parse::<_, $error>(s, context)?
                    };

                    $($validate(name, &value)?;)?
                    Ok(Some(value))
                }
            )*
        }
    };
}

#[inline(never)]
#[doc(hidden)]
pub fn parse<T: FromParamStr, E>(
    s: impl AsRef<str>,
    context: &mut impl BuildContext,
) -> Result<T, E>
where
    <T as FromParamStr>::Err: Into<E>,
{
    match T::from_param_str(s.as_ref(), context) {
        Ok(value) => Ok(value),
        Err(e) => Err(e.into()),
    }
}

#[inline(never)]
#[doc(hidden)]
pub fn get_envs(
    names: &'static [&'static str],
    context: &mut impl BuildContext,
) -> Result<Option<(&'static str, Cow<'static, str>)>, std::env::VarError> {
    let mut value = None;
    let mut found_vars = Vec::new();

    for name in names {
        match context.env().read(name) {
            Ok(val) => {
                found_vars.push(format!("{}={}", name, val));
                if value.is_none() {
                    value = Some((*name, Cow::Owned(val.to_string())));
                }
            }
            Err(std::env::VarError::NotPresent) => continue,
            Err(err @ std::env::VarError::NotUnicode(_)) => {
                return Err(err);
            }
        }
    }

    if found_vars.len() > 1 {
        context.warn(format!(
            "Multiple environment variables set: {}",
            found_vars.join(", ")
        ));
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, convert::Infallible};

    #[derive(Debug)]
    enum Error {
        VarError,
    }

    impl From<Infallible> for Error {
        fn from(_: Infallible) -> Self {
            unreachable!()
        }
    }

    impl From<std::env::VarError> for Error {
        fn from(_error: std::env::VarError) -> Self {
            Error::VarError
        }
    }

    use crate::gel::BuildContextImpl;

    use super::define_env;
    define_env! {
        type Error = Error;

        #[doc="The host to connect to."]
        #[env(GEL_HOST, EDGEDB_HOST)]
        host: String,
    }

    #[test]
    fn test_define_env() {
        let map = HashMap::from([("GEL_HOST", "localhost"), ("EDGEDB_HOST", "localhost")]);
        let mut context = BuildContextImpl::new_with(&map, ());
        assert_eq!(
            Env::host(&mut context).unwrap(),
            Some("localhost".to_string())
        );
        assert_eq!(
            context.warnings.warnings,
            vec!["Multiple environment variables set: GEL_HOST=localhost, EDGEDB_HOST=localhost"]
        );
    }
}
