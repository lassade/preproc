//! Handle `#include`, `#if` and `#define` `#undef` directives in any source file

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

/// Raw lines sent by the
#[derive(Debug, PartialEq, Eq)]
pub enum RawLine<'a> {
    /// A normal line of code
    Code(&'a str),
    /// A line of code that starts with a user defined ascii char (usually `b'#'`)
    Directive(&'a str, Option<&'a str>), // todo: include the entire line as the last argument
}

#[inline(always)]
const unsafe fn str_from_raw_parts<'a>(ptr: *const u8, len: usize) -> &'a str {
    core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len))
}

#[inline(always)]
const unsafe fn str_from_range<'a>(ptr: *const u8, ptr_end: *const u8) -> &'a str {
    str_from_raw_parts(ptr, ptr_end.offset_from(ptr) as usize)
}

#[derive(Debug, PartialEq, Eq)]
pub enum Line<'a> {
    Code(&'a str),
    Inc(&'a str),
    Def(&'a str),
    Undef(&'a str),
    If(Exp<'a>),
    Elif(Exp<'a>),
    Else,
    Endif,
}

#[derive(Default)]
pub struct File<'a> {
    pub lines: Vec<Line<'a>>,
}

impl<'a> File<'a> {
    pub fn from_str(input: &'a str, config: &Config) -> Self {
        let mut file = Self::default();
        sse2::parse_file(input, config, |line| match line {
            RawLine::Code(code) => file.lines.push(Line::Code(code)),
            RawLine::Directive(directive, arg) => {
                file.lines.push(sse2::parse_directive(directive, arg))
            }
        });
        file
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    const FILES: &[&str] = &[
        //"benches/files/Native.g.cs",
        "benches/files/shader.wgsl",
    ];

    #[test]
    fn basic() {
        let config = Config::default();

        for path in FILES {
            let input = std::fs::read_to_string(path).expect("file not found");
            //File::from_str(&input, &config);

            // used to create the source of truth
            let mut output =
                std::fs::File::create(format!("{}.t", path)).expect("failed to create output file");
            sse2::parse_file(&input, &config, |line| {
                writeln!(output, "{:?}", &line).unwrap();
            });
        }
    }
}
