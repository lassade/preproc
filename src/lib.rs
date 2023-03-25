//! Handle `#include`, `#if` and `#define` `#undef` directives in any source file

use std::{
    fs,
    path::{Path, PathBuf},
    str::{self},
};

use chars::Chars;
use codespan_reporting::diagnostic::{Diagnostic, Label};
use hashbrown::HashSet;
use simdutf8::basic::from_utf8;

mod chars;
mod exp;
mod sse2;

type Result<T> = core::result::Result<T, Diagnostic<usize>>;

#[derive(Default)]
pub struct PP {
    search_paths: Vec<PathBuf>,
    defines: HashSet<String>,
}

impl PP {
    pub fn search_path(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.search_paths.push(path.into());
        self
    }

    pub fn define(&mut self, define: &str) -> &mut Self {
        self.defines.insert(define.to_string());
        self
    }

    pub fn undef(&mut self, define: &str) -> &mut Self {
        self.defines.remove(define);
        self
    }

    pub fn parse_file(&self, input_file: impl AsRef<Path>) -> String {
        let bytes = fs::read(input_file).expect("failed to read file");
        self.parse_str(from_utf8(&bytes).expect("only support utf8"))
    }

    pub fn parse_str(&self, input: &str) -> String {
        let mut output = String::default();
        self.parse_internal(input, &mut output);
        output
    }

    fn parse_internal(&self, input: &str, output: &mut String) {
        // process all the file
        let mut text: Chars = input.into();
        let mut expressions = vec![];
        let mut scratch = vec![];
        while text.head().is_some() {
            match parse_line(&mut text).expect("not error") {
                Event::Include(path) => {
                    // todo: cache previous included files
                    let mut found = false;
                    for search_path in &self.search_paths {
                        let path = search_path.join(path);

                        if let Ok(bytes) = fs::read(path) {
                            self.parse_internal(
                                from_utf8(&bytes).expect("only support utf8"),
                                output,
                            );
                            found = true;
                            break;
                        }
                    }

                    if !found {
                        panic!("file not found");
                    }
                }
                Event::Code(code) => {
                    if Some(false) == expressions.last().copied() {
                        // ignore
                    } else {
                        output.push_str(code);
                    }
                }
                Event::If(exp) => {
                    expressions.push(
                        self.eval(&mut scratch, &exp[..])
                            .expect("something is wrong"),
                    );
                }
                Event::ElseIf(exp) => {
                    expressions.pop();
                    expressions.push(
                        self.eval(&mut scratch, &exp[..])
                            .expect("something is wrong"),
                    );
                }
                Event::Else => {
                    if let Some(b) = expressions.pop() {
                        expressions.push(!b);
                    }
                }
                Event::EndIf => {
                    expressions.pop();
                }
                Event::EOF => {
                    break;
                }
            }
        }
    }

    fn eval<'a>(&self, scratch: &mut Vec<bool>, exp: &[Exp<'a>]) -> Option<bool> {
        scratch.clear();

        for op in exp {
            match op {
                Exp::Name(define) => scratch.push(self.defines.contains(*define)),
                Exp::And => {
                    let a = scratch.pop()?;
                    let b = scratch.pop()?;
                    let r = a && b;
                    scratch.push(r);
                }
                Exp::Or => {
                    let a = scratch.pop()?;
                    let b = scratch.pop()?;
                    let r = a || b;
                    scratch.push(r);
                }
                Exp::Not => {
                    let r = !(scratch.pop()?);
                    scratch.push(r);
                }
            }
        }

        let result = scratch.pop();
        assert!(scratch.len() == 0, "malformed expression {:?}", exp);
        result
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
    Code(&'a str),
    Include(&'a str),
    If(Vec<Exp<'a>>),
    ElseIf(Vec<Exp<'a>>),
    Else,
    EndIf,
    EOF,
}

fn parse_line<'a>(text: &mut Chars<'a>) -> Result<Event<'a>> {
    let line_cursor = text.cursor();

    ignore_spaces(text);

    match text.head() {
        Some('#') => {
            match directive(text) {
                "#include" => include(text),
                "#if" => Ok(Event::If(exp(text))),
                "#elif" => Ok(Event::ElseIf(exp(text))),
                "#else" => {
                    next_line(text);
                    Ok(Event::Else)
                }
                "#endif" => {
                    next_line(text);
                    Ok(Event::EndIf)
                }
                _ => {
                    // ignores unknow directive
                    next_line(text);
                    Ok(Event::Code(text.sub_str_from_cursor(line_cursor)))
                }
            }
        }
        Some(_) => {
            // no more directives can be found in here skip to the next line
            next_line(text);
            Ok(Event::Code(text.sub_str_from_cursor(line_cursor)))
        }
        None => Ok(Event::EOF),
    }
}

fn directive<'a>(text: &mut Chars<'a>) -> &'a str {
    let directive_cursor = text.cursor();

    // read until the next whitespace or enter
    while let Some(ch) = text.head() {
        if ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' {
            break;
        } else {
            text.next();
        }
    }

    text.sub_str_from_cursor(directive_cursor)
}

fn ignore_spaces(text: &mut Chars) {
    while let Some(ch) = text.head() {
        if ch == ' ' || ch == '\t' {
            text.next();
        } else {
            break;
        }
    }
}

fn next_line(text: &mut Chars) {
    while let Some(ch) = text.head() {
        text.next();
        match ch {
            '\n' | '\r' => {
                break;
            }
            _ => {}
        }
    }
}

fn include<'a>(text: &mut Chars<'a>) -> Result<Event<'a>> {
    ignore_spaces(text);

    let delimiter = match text.head() {
        Some('\"') => '\"',
        Some('<') => '>',
        _ => {
            let offset = text.offset_from_source_str();
            let d = Diagnostic::error()
                .with_message("expecting `\"` or `<`")
                .with_labels(vec![Label::primary(0, offset..offset)]);
            return Err(d);
        }
    };

    text.next();

    let path_cursor = text.cursor();
    let path_offset = text.offset_from_source_str();

    while let Some(ch) = text.head() {
        if ch == delimiter {
            return Ok(Event::Include(text.sub_str_from_cursor(path_cursor)));
        }
        match ch {
            '\n' | '\r' => {
                let offset = text.offset_from_source_str();
                let d = Diagnostic::error()
                    .with_message(format!("expecting `{}`", delimiter))
                    .with_labels(vec![Label::primary(0, offset..offset)
                        .with_message(format!("expecting `{}`", delimiter))]);
                return Err(d);
            }
            _ => {}
        }

        text.next();
    }

    let offset = text.offset_from_source_str();
    let d = Diagnostic::error()
        .with_message("reached end of file")
        .with_labels(vec![Label::primary(0, path_offset..offset)
            .with_message(format!("expecting `{}`", delimiter))]);
    return Err(d);
}

fn exp<'a>(text: &mut Chars<'a>) -> Vec<Exp<'a>> {
    let mut exp = vec![];
    exp_internal(text, &mut exp);
    exp
}

fn exp_internal<'a>(text: &mut Chars<'a>, exp: &mut Vec<Exp<'a>>) {
    ignore_spaces(text);

    let mut op = 0;
    while let Some(ch) = text.head() {
        match ch {
            '(' => {
                text.next();
            }
            '|' => {
                text.next();
                if op == 1 {
                    exp_internal(text, exp);
                    exp.push(Exp::Or);
                    op = 0;
                } else {
                    op = 1;
                }
            }
            '&' => {
                text.next();
                if op == 2 {
                    exp_internal(text, exp);
                    exp.push(Exp::And);
                    op = 0;
                } else {
                    op = 2;
                }
            }
            '!' => {
                text.next();

                // find if the expressions should be groupped or not
                match text.head() {
                    Some('(') => exp_internal(text, exp),
                    _ => exp_name(text, exp),
                }

                exp.push(Exp::Not);
                op = 0;
            }
            ')' | '\n' | '\r' => {
                text.next();
                return;
            }
            ch => {
                if ch.is_ascii_alphanumeric() {
                    exp_name(text, exp);
                } else {
                    return;
                }
            }
        }
    }
}

fn exp_name<'a>(text: &mut Chars<'a>, exp: &mut Vec<Exp<'a>>) {
    ignore_spaces(text);

    let name;
    let name_cursor = text.cursor();

    loop {
        if let Some(ch) = text.head() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                text.next();
                continue;
            } else {
                name = text.sub_str_from_cursor(name_cursor);
                if ch == ' ' || ch == '\t' {
                    text.next();
                }

                break;
            }
        } else {
            name = text.sub_str_from_cursor(name_cursor);
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
    fn conditional() {
        let input = r#"#if SHADOWS // some comment
fn func() -> f32 {
    return 1.0;
}
#else
fn func() -> f32 {
    return 0.0;
}
#endif"#;

        assert_eq!(
            PP::default().define("SHADOWS").parse_str(input),
            r#"// some comment
fn func() -> f32 {
    return 1.0;
}
"#
        );
    }

    // fn include() {
    //     let input = r#"#include "somefile.wgsl" // comment

    //     #if SHADOWS // some comment
    //     fn func() -> f32 {
    //         return 1.0;
    //     }
    //     #else
    //     fn func() -> f32 {
    //         return 0.0;
    //     }
    //     #endif"#;

    //     println!("{}", PP::default().define("SHADOWS").parse_str(input));
    // }
}
