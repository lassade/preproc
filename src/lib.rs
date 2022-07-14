//! Handle `#include`, `#if` and `#define` `#undef` directives in any source file

use std::{
    fs,
    path::{Path, PathBuf},
};

use ahash::AHashSet;
use codespan_reporting::diagnostic::{Diagnostic, Label};
use simdutf8::basic::from_utf8;

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

#[derive(Debug)]
enum Event<'a> {
    Code,
    Include(&'a str),
    If(&'a str),
    ElseIf(&'a str),
    Else,
    EndIf,
    Define(&'a str),
    Undef(&'a str),
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
                        let (r, next) = expression(j, bytes);
                        return (r.map(Event::If), next);
                    }
                    "#elif" => {
                        let (r, next) = expression(j, bytes);
                        return (r.map(Event::ElseIf), next);
                    }
                    "#else" => {
                        return (Ok(Event::Else), next_line(j, bytes));
                    }
                    "#endif" => {
                        return (Ok(Event::EndIf), next_line(j, bytes));
                    }
                    "#define" => {
                        let (r, next) = name(j, bytes);
                        return (r.map(Event::Define), next);
                    }
                    "#undef" => {
                        let (r, next) = name(j, bytes);
                        return (r.map(Event::Undef), next);
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

fn next_line(offset: usize, bytes: &[u8]) -> usize {
    for i in offset..bytes.len() {
        match bytes[i] {
            b'\n' | b'\r' => {
                return i + 1;
            }
            _ => {}
        }
    }
    // bytes eneded
    return bytes.len();
}

#[inline]
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

#[inline]
fn name(offset: usize, bytes: &[u8]) -> (Result<&str>, usize) {
    let offset = ignore_spaces(offset, bytes);

    for j in offset..bytes.len() {
        let b = bytes[j];
        match b {
            b'\t' | b' ' | b'/' | b'*' | b'\n' | b'\r' => {
                let name = from_utf8(&bytes[offset..j]).expect("not utf8");
                return (Ok(name), next_line(j, bytes));
            }
            _ => {}
        }
    }

    let name = from_utf8(&bytes[offset..bytes.len()]).expect("not utf8");
    return (Ok(name), bytes.len());
}

pub enum Op {
    Define(String),
    And,
    Or,
    Not,
}

#[inline]
fn expression(offset: usize, bytes: &[u8]) -> (Result<&str>, usize) {
    (Ok("?"), next_line(offset, bytes))
    // for i in offset..bytes.len() {
    //     match bytes[i] {
    //         b'\n' | b'\r' => {
    //             return i + 1;
    //         }
    //         _ => {}
    //     }
    // }
    // // bytes eneded
    // return bytes.len();
}

#[cfg(test)]
mod tests {
    use super::*;

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
