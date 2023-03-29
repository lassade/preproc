//! Handle `#include`, `#if` and `#define` `#undef` directives in any source file

use core::fmt;

mod chars;
pub mod exp;
pub mod sse2;

use exp::Exp;

pub struct Config {
    /// Special ASCII character used to define the start of and directive, default is `b'#'`
    /// but is possible to configure to something like `b'@'`, `b'%'` or `b'!'`
    pub special_char: u8,
    // /// Single line comment string, default "//"
    // pub comment: &'a str,
    // /// Start of a multi-line comment, default "/*"
    // pub comment_begin: &'a str,
    // /// End of a multi-line comment, default "*/"
    // pub comment_end: &'a str,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            special_char: b'#',
            // comment: "//",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Val<'a> {
    Raw(&'a str), // todo: remove
    Str(&'a str),
    Exp(Exp<'a>),
}

impl<'a> fmt::Display for Val<'a> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Val::Raw(s) => write!(f, "{}", s),
            Val::Str(path) => write!(f, "\"{}\"", path),
            Val::Exp(exp) => write!(f, "{}", exp),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Line<'a> {
    Code(&'a str),
    Directive(&'a str, Option<Val<'a>>),
}

#[derive(Default)]
pub struct File<'a> {
    pub lines: Vec<Line<'a>>,
}

#[inline(always)]
const unsafe fn str_from_raw_parts<'a>(ptr: *const u8, len: usize) -> &'a str {
    core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len))
}

#[inline(always)]
const unsafe fn str_from_range<'a>(ptr: *const u8, ptr_end: *const u8) -> &'a str {
    str_from_raw_parts(ptr, ptr_end.offset_from(ptr) as usize)
}
