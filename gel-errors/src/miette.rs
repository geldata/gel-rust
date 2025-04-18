//! Miette support for Gel errors. Add "miette" feature flag to enable.
//!
//! [miette](https://docs.rs/miette/latest/miette/) allows nice error formatting via its [Diagnostic](https://docs.rs/miette/latest/miette/trait.Diagnostic.html) trait
//!
use miette::{LabeledSpan, SourceCode};
use std::fmt::Display;

use crate::fields::QueryText;
use crate::Error;

impl miette::Diagnostic for Error {
    fn code(&self) -> Option<Box<dyn Display + '_>> {
        Some(Box::new(self.kind_name()))
    }
    fn source_code(&self) -> Option<&dyn SourceCode> {
        self.get::<QueryText>().map(|s| s as _)
    }
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        let (start, end) = self.position_start().zip(self.position_end())?;
        let len = end - start;
        Some(Box::new(
            Some(LabeledSpan::new(self.hint().map(Into::into), start, len)).into_iter(),
        ))
    }
    fn help(&self) -> Option<Box<dyn Display + '_>> {
        self.details().map(|v| Box::new(v) as Box<dyn Display>)
    }
}
