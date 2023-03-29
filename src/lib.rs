//! Handle `#include`, `#if` and `#define` `#undef` directives in any source file

use core::fmt;

mod chars;
pub mod exp;
mod sse2;

use exp::Exp;

#[derive(Debug, PartialEq, Eq)]
pub enum Val<'a> {
    Path(&'a str),
    Exp(Exp<'a>),
}

impl<'a> fmt::Display for Val<'a> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Val::Path(path) => write!(f, "\"{}\"", path),
            Val::Exp(exp) => write!(f, "{}", exp),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Line<'a> {
    Code(&'a str),
    Directive(&'a str, Option<Val<'a>>),
    EOF,
}
