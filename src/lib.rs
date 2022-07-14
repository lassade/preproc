//! Handle `#include`, `#if` and `#define` `#undef` directives in any source file

use std::{
    fs,
    path::{Path, PathBuf},
    str,
};

use ahash::AHashSet;
use codespan_reporting::diagnostic::{Diagnostic, Label};
use simdutf8::basic::from_utf8;

mod chars;

// 1. load file
// 2. split into lines
// 3. find and replace #include directives
// 4. evaluate ifdef, ifndef and define expressions

type Result<T> = core::result::Result<T, Diagnostic<usize>>;

#[derive(Default)]
pub struct PP {
    /// Search paths for include files
    search_paths: Vec<PathBuf>,
    /// Current list of defines
    defines: AHashSet<String>,
}

impl PP {
    pub fn search_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.search_paths.push(path.into());
        self
    }

    pub fn define(mut self, define: &str) -> Self {
        self.defines.insert(define.to_string());
        self
    }

    pub fn undef(mut self, define: &str) -> Self {
        self.defines.remove(define);
        self
    }

    pub fn parse_str(&self, input: &str) {
        let bytes = input.as_bytes();
        // process all the file
        let mut line = 0;
        while line < bytes.len() {
            let (evn, next) = parse(line, &bytes);
            dbg!(evn);
            line = next;
        }
    }

    pub fn parse_file(&self, input_file: impl AsRef<Path>) {
        let bytes = fs::read(input_file).expect("failed to read file");
        self.parse_str(from_utf8(&bytes).expect("only support utf8"));
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Exp<'a> {
    Name(&'a str),
    And,
    Or,
    Not,
}

#[derive(Debug)]
enum Event<'a> {
    Code,
    Include(&'a str),
    If(Vec<Exp<'a>>),
    ElseIf(Vec<Exp<'a>>),
    Else,
    EndIf,
}

fn parse(offset: usize, bytes: &[u8]) -> (Result<Event>, usize) {
    for i in offset..bytes.len() {
        match bytes[i] {
            b'\n' | b'\r' => {
                // next line
                return (Ok(Event::Code), i + 1);
            }
            b'\t' | b' ' => {} // just move forward
            b'#' => {
                let j = next(i, bytes);
                let directive = from_utf8(&bytes[i..j]).expect("not utf8");
                match directive {
                    "#include" => {
                        return include(j, bytes);
                    }
                    "#if" => {
                        let (r, next) = exp(j, bytes);
                        return (Ok(Event::If(r)), next);
                    }
                    "#elif" => {
                        let (r, next) = exp(j, bytes);
                        return (Ok(Event::ElseIf(r)), next);
                    }
                    "#else" => {
                        return (Ok(Event::Else), next_line(j, bytes));
                    }
                    "#endif" => {
                        return (Ok(Event::EndIf), next_line(j, bytes));
                    }
                    _ => {
                        // ignores unknow directive
                        return (Ok(Event::Code), next_line(j, bytes));
                    }
                }
            }
            _ => {
                // no more directives can be found in here skip to the next line
                return (Ok(Event::Code), next_line(i, bytes));
            }
        }
    }
    return (Ok(Event::Code), bytes.len());
}

fn next(offset: usize, bytes: &[u8]) -> usize {
    for i in offset..bytes.len() {
        match bytes[i] {
            b'\t' | b' ' | b'\n' | b'\r' => {
                return i;
            }
            _ => {}
        }
    }
    // bytes eneded
    return bytes.len();
}

fn ignore_spaces(offset: usize, bytes: &[u8]) -> usize {
    for i in offset..bytes.len() {
        match bytes[i] {
            b'\t' | b' ' => {}
            _ => {
                return i;
            }
        }
    }
    // bytes eneded
    return bytes.len();
}

fn next_line(mut offset: usize, bytes: &[u8]) -> usize {
    loop {
        if offset >= bytes.len() {
            break;
        }

        let byte = bytes[offset];
        let len = utf8_byte_count(byte);
        offset += len;

        // ascii only
        if len != 1 {
            continue;
        }

        match byte {
            b'\n' | b'\r' => {
                return offset;
            }
            _ => {}
        }
    }

    // bytes eneded
    return offset;
}

fn include(offset: usize, bytes: &[u8]) -> (Result<Event>, usize) {
    let offset = ignore_spaces(offset, bytes);
    match bytes.get(offset) {
        Some(b'\"') => delimited_name(offset + 1, bytes, b'\"'),
        Some(b'<') => delimited_name(offset + 1, bytes, b'>'),
        _ => {
            let d = Diagnostic::error()
                .with_message("expecting `\"` or `<`")
                .with_labels(vec![Label::primary(0, offset..offset)]);
            return (Err(d), 0);
        }
    }
}

fn delimited_name(offset: usize, bytes: &[u8], delimiter: u8) -> (Result<Event>, usize) {
    let mut slash = false;
    for j in offset..bytes.len() {
        let b = bytes[j];
        if b == delimiter {
            let include_file = from_utf8(&bytes[offset..j]).expect("not utf8");
            return (Ok(Event::Include(include_file)), next_line(j, bytes));
        }
        match b {
            b'/' => {
                if slash {
                    let d = Diagnostic::error()
                        .with_message("expecting `\"` or `>`")
                        .with_labels(vec![Label::primary(0, j - 1..j)]);
                    return (Err(d), next_line(j, bytes));
                }
                slash = true;
            }
            b'*' => {
                if slash {
                    panic!("multiline comments not supported yet")
                    // return Err(Diagnostic::error().with_message("expecting `\"` or `>`").with_labels(vec![Label::primary(0, j-1..j)]));
                }
                slash = false;
            }
            b'\n' | b'\r' => {
                let d = Diagnostic::error()
                    .with_message("expecting `\"` or `>`")
                    .with_labels(vec![Label::primary(0, offset..bytes.len())]);
                return (Err(d), j + 1);
            }
            _ => {
                slash = false;
            }
        }
    }

    let d = Diagnostic::error()
        .with_message("end of file reached, expecting `\"` or `>`")
        .with_labels(vec![Label::primary(0, offset..bytes.len())]);
    return (Err(d), bytes.len());
}

fn exp<'a>(mut offset: usize, bytes: &'a [u8]) -> (Vec<Exp<'a>>, usize) {
    let mut exp = vec![];
    exp_inner(&mut offset, bytes, &mut exp);
    (exp, offset)
}

fn exp_inner<'a>(offset: &mut usize, bytes: &'a [u8], exp: &mut Vec<Exp<'a>>) {
    let mut op = 0;
    loop {
        if *offset >= bytes.len() {
            break;
        }

        let byte = bytes[*offset];
        let len = utf8_byte_count(byte);

        // ascii only
        if len != 1 {
            *offset = *offset + len;
            continue;
        }

        match byte {
            b'(' => {
                *offset = *offset + 1;
                exp_name(offset, bytes, exp);
            }
            b'|' => {
                *offset = *offset + 1;
                if op == 1 {
                    exp_inner(offset, bytes, exp);
                    exp.push(Exp::Or);
                    op = 0;
                } else {
                    op = 1;
                }
            }
            b'&' => {
                *offset = *offset + 1;
                if op == 2 {
                    exp_inner(offset, bytes, exp);
                    exp.push(Exp::And);
                    op = 0;
                } else {
                    op = 2;
                }
            }
            b'!' => {
                *offset = *offset + 1;
                exp_inner(offset, bytes, exp);
                exp.push(Exp::Not);
                op = 0;
            }
            b'/' => {
                *offset = next_line(*offset, bytes);
                return;
            }
            b')' | b'\n' | b'\r' => {
                *offset = *offset + 1;
                return;
            }
            _ => {
                exp_name(offset, bytes, exp);
            }
        }
    }
}

fn exp_name<'a>(offset: &mut usize, bytes: &'a [u8], exp: &mut Vec<Exp<'a>>) {
    *offset = ignore_spaces(*offset, bytes);

    let n = *offset;
    let slice;
    loop {
        if *offset == bytes.len() {
            // take the remainder
            slice = &bytes[n..(*offset)];
            break;
        }
        assert!(*offset < bytes.len(), "worng encoding");

        let byte = bytes[*offset];
        let len = utf8_byte_count(byte);

        // ascii only
        if len != 1 {
            *offset = *offset + len;
            continue;
        }

        if (b'a' <= byte && byte <= b'z')
            || (b'A' <= byte && byte <= b'Z')
            || (b'0' <= byte && byte <= b'9')
            || byte == b'_'
        {
            *offset = *offset + len;
            continue;
        }

        // take the current
        slice = &bytes[n..(*offset)];

        if byte == b' ' || byte == b'\t' || byte == b'\n' || byte == b'\r' {
            *offset = *offset + 1;
        }

        break;
    }

    if slice.len() > 0 {
        unsafe {
            exp.push(Exp::Name(str::from_utf8_unchecked(slice)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions() {
        assert_eq!(&(exp(0, b"a").0), &[Exp::Name("a")]);
        assert_eq!(&(exp(0, b"!a").0), &[Exp::Name("a"), Exp::Not]);
        assert_eq!(
            &(exp(0, b"a || b").0),
            &[Exp::Name("a"), Exp::Name("b"), Exp::Or]
        );
        assert_eq!(
            &(exp(0, b"a && b").0),
            &[Exp::Name("a"), Exp::Name("b"), Exp::And]
        );
        assert_eq!(
            &(exp(0, b"!a && b").0),
            &[Exp::Name("a"), Exp::Not, Exp::Name("b"), Exp::And]
        );
        assert_eq!(
            &(exp(0, b"!a && !b").0),
            &[Exp::Name("a"), Exp::Not, Exp::Name("b"), Exp::Not, Exp::And]
        );
        assert_eq!(
            &(exp(0, b"a && !b").0),
            &[Exp::Name("a"), Exp::Name("b"), Exp::Not, Exp::And]
        );

        // dbg!(exp(0, b"c || !(a && b)").0);
        // dbg!(exp(0, b"c || !a && b").0);
    }

    #[test]
    fn it_works() {
        let input = r#"
        #include "somefile.wgsl" // comment

        #if SHADOWS // some comment
        fn func() -> f32 {
            return 1.0;
        }
        #else
        fn func() -> f32 {
            return 0.0;
        }
        #endif
        "#;

        PP::default().define("SHADOWS").parse_str(input);
    }
}
