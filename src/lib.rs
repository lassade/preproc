//! Handle `#include`, `#if` and `#define` `#undef` directives in any source file

use core::slice;
use std::{
    fs,
    path::{Path, PathBuf},
    str,
};

use ahash::AHashSet;
use chars::Chars;
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
                        todo!();
                        // let (r, next) = exp(j, bytes);
                        // return (Ok(Event::If(r)), next);
                    }
                    "#elif" => {
                        todo!();
                        // let (r, next) = exp(j, bytes);
                        // return (Ok(Event::ElseIf(r)), next);
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

fn ignore_spaces(text: &mut Chars) {
    while let Some(ch) = text.peek() {
        match ch {
            '\t' | ' ' => {
                text.next();
            }
            _ => {
                break;
            }
        }
    }
}

fn next_line(mut offset: usize, bytes: &[u8]) -> usize {
    // loop {
    //     if offset >= bytes.len() {
    //         break;
    //     }

    //     let byte = bytes[offset];
    //     let len = utf8_byte_count(byte);
    //     offset += len;

    //     // ascii only
    //     if len != 1 {
    //         continue;
    //     }

    //     match byte {
    //         b'\n' | b'\r' => {
    //             return offset;
    //         }
    //         _ => {}
    //     }
    // }

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

fn exp<'a>(text: &mut Chars<'a>) -> Vec<Exp<'a>> {
    let mut exp = vec![];
    let next = exp_inner(text, &mut exp);
    exp
}

fn exp_inner<'a>(text: &mut Chars<'a>, exp: &mut Vec<Exp<'a>>) {
    let mut op = 0;
    while let Some(ch) = text.peek() {
        match ch {
            '(' => {
                text.next();
            }
            '|' => {
                text.next();
                if op == 1 {
                    exp_inner(text, exp);
                    exp.push(Exp::Or);
                    op = 0;
                }
                op = 1;
            }
            '&' => {
                text.next();
                if op == 2 {
                    exp_inner(text, exp);
                    exp.push(Exp::And);
                    op = 0;
                }
                op = 2;
            }
            '!' => {
                text.next();

                // find if the expressions should be groupped or not
                ignore_spaces_alt(text);
                match text.peek() {
                    Some('(') => exp_inner(text, exp),
                    _ => exp_name(text, exp),
                }

                exp.push(Exp::Not);
                op = 0;
            }
            ')' | '\n' | '\r' => {
                text.next();
                return;
            }
            _ => {
                exp_name(text, exp);
            }
        }
    }
}

fn exp_name<'a>(text: &mut Chars<'a>, exp: &mut Vec<Exp<'a>>) {
    ignore_spaces_alt(text);

    let ptr = text.as_ptr();
    let name;

    loop {
        if let Some(ch) = text.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                text.next();
                continue;
            } else {
                name = unsafe {
                    str::from_utf8_unchecked(slice::from_raw_parts(
                        ptr,
                        text.as_ptr().offset_from(ptr) as _,
                    ))
                };

                if ch.is_whitespace() {
                    text.next();
                }

                break;
            }
        } else {
            name = unsafe {
                str::from_utf8_unchecked(slice::from_raw_parts(
                    ptr,
                    text.as_ptr().offset_from(ptr) as _,
                ))
            };

            break;
        }
    }

    if name.len() > 0 {
        exp.push(Exp::Name(name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions() {
        fn test_exp<'a>(text: &'a str) -> Vec<Exp<'a>> {
            let mut text: Chars = text.into();
            super::exp(&mut text)
        }

        assert_eq!(&test_exp("a"), &[Exp::Name("a")]);
        assert_eq!(&test_exp("!a"), &[Exp::Name("a"), Exp::Not]);
        assert_eq!(
            &test_exp("a || b"),
            &[Exp::Name("a"), Exp::Name("b"), Exp::Or]
        );
        assert_eq!(
            &test_exp("a && b"),
            &[Exp::Name("a"), Exp::Name("b"), Exp::And]
        );
        assert_eq!(
            &test_exp("!a && b"),
            &[Exp::Name("a"), Exp::Not, Exp::Name("b"), Exp::And]
        );
        assert_eq!(
            &test_exp("!a && !b"),
            &[Exp::Name("a"), Exp::Not, Exp::Name("b"), Exp::Not, Exp::And]
        );
        assert_eq!(
            &test_exp("a && !b"),
            &[Exp::Name("a"), Exp::Name("b"), Exp::Not, Exp::And]
        );
        assert_eq!(
            &test_exp("a && (!b || c)"),
            &[
                Exp::Name("a"),
                Exp::Name("b"),
                Exp::Not,
                Exp::Name("c"),
                Exp::Or,
                Exp::And
            ]
        );
        assert_eq!(
            &test_exp("a && (((!b || c)))"),
            &[
                Exp::Name("a"),
                Exp::Name("b"),
                Exp::Not,
                Exp::Name("c"),
                Exp::Or,
                Exp::And
            ]
        );

        // dbg!(exp(0, b"c || !(a && b)").0);
        // dbg!(exp(0, b"c || !a && b").0);
    }

    #[test]
    fn src_input() {
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
