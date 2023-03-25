// todo: change to alloc
use core::fmt;

use beef::Cow;
use hashbrown::HashMap;
use smallvec::SmallVec;

/// Operands and Operators
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Op<'a> {
    Var(&'a str),
    And,
    Or,
    Not,
}

impl<'a> Op<'a> {
    pub fn from_str(op: &'a str) -> Self {
        match op {
            "&&" => Op::And,
            "||" => Op::Or,
            "!" => Op::Not,
            var => Op::Var(var),
        }
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Ctx {
    pub vars: HashMap<String, bool>,
    stack: Vec<bool>,
}

#[inline(always)]
const unsafe fn str_from_raw_parts<'a>(ptr: *const u8, len: usize) -> &'a str {
    core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len))
}

/// Supported operators, the slice order gives the precedence
const OPERATORS: &'static [&'static str] = &["&&", "||", "!"];

/// Break chars, must be sorted
const BREAK: &'static [u8] = &[b'\t', b' ', b'!', b'&', b'(', b')', b'|'];

/// Expression, internally it uses the Reverse Polish Notation (RPN) notation
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Exp<'a> {
    pub ops: Vec<Op<'a>>,
}

impl<'a> Exp<'a> {
    // todo: really bad performance
    pub fn from_str(exp: &'a str) -> Self {
        // uses the shunting yard algorithm
        // https://en.wikipedia.org/wiki/Shunting_yard_algorithm

        let mut stack: SmallVec<[&'a str; 16]> = SmallVec::new();
        let mut output = vec![];

        let data = exp.as_bytes();
        let mut ptr = data.as_ptr();
        let mut token_ptr = ptr;
        let ptr_end = unsafe { ptr.add(data.len()) };

        loop {
            if ptr >= ptr_end {
                break;
            }

            let ch;
            unsafe {
                ch = *ptr;
                ptr = ptr.add(1);
            }

            // only process ascii chars
            if ch > 127 {
                continue;
            }

            // safety: str slice respect the utf8 chars continuation bytes, because it will only split in ascii chars
            let token =
                unsafe { str_from_raw_parts(token_ptr, ptr.offset_from(token_ptr) as usize) };

            if token.len() == 1 {
                if ch == b' ' || ch == b'\t' {
                    // accept and skip whitespace tokens
                    token_ptr = ptr;
                    continue;
                } else if ch == b'&' || ch == b'|' {
                    // just don't accept these as tokens
                    continue;
                }
            }

            if token == "(" {
                token_ptr = ptr; // accept the token
                stack.push(token);
            } else if token == ")" {
                token_ptr = ptr; // accept the token
                while let Some(val) = stack.last().copied() {
                    if val == "(" {
                        stack.pop();
                        break;
                    } else {
                        output.push(Op::from_str(stack.pop().unwrap()));
                    }
                }
            } else if let Some(i) = OPERATORS.iter().position(|&r| r == token) {
                token_ptr = ptr; // accept the token
                loop {
                    if stack.is_empty() {
                        break;
                    }

                    if let Some(j) = OPERATORS.iter().position(|&r| r == *stack.last().unwrap()) {
                        if i <= j {
                            output.push(Op::from_str(stack.pop().unwrap()));
                            continue;
                        }
                    }

                    break;
                }
                stack.push(token);
            } else {
                if ptr >= ptr_end {
                    // accept the token
                } else {
                    // fetch the next char
                    // safety:  ptr is within `str`, bounds
                    let ch = unsafe { *ptr };
                    if BREAK.binary_search(&ch).is_err() {
                        // continue appending more chars
                        continue;
                    }
                }

                token_ptr = ptr; // accept the token
                output.push(Op::from_str(token));
            }
        }

        while let Some(token) = stack.pop() {
            if token == "(" {
                continue;
            }

            output.push(Op::from_str(token));
        }

        Self { ops: output }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn is_valid(&self) -> bool {
        let mut stack_depth = 0;

        for op in &self.ops {
            match op {
                Op::Var(_) => stack_depth += 1,
                Op::Or | Op::And => stack_depth -= 1,
                _ => {}
            }
        }

        stack_depth == 1
    }

    pub fn eval(&self, ctx: &mut Ctx) -> bool {
        ctx.stack.clear();

        for op in &self.ops {
            match op {
                Op::Var(var) => ctx
                    .stack
                    .push(ctx.vars.get(*var).copied().unwrap_or_default()),
                Op::And => {
                    if ctx.stack.len() < 2 {
                        panic!("malformed exp");
                    }
                    let b = ctx.stack.pop().unwrap();
                    let a = ctx.stack.pop().unwrap();
                    ctx.stack.push(a && b);
                }
                Op::Or => {
                    if ctx.stack.len() < 2 {
                        panic!("malformed exp");
                    }
                    let b = ctx.stack.pop().unwrap();
                    let a = ctx.stack.pop().unwrap();
                    ctx.stack.push(a || b);
                }
                Op::Not => {
                    let a = ctx.stack.pop().expect("malformed exp");
                    ctx.stack.push(!a);
                }
            }
        }

        if ctx.stack.len() != 1 {
            panic!("malformed exp");
        }

        ctx.stack.pop().unwrap()
    }
}

impl<'a> fmt::Display for Exp<'a> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut stack: SmallVec<[Cow<'a, str>; 16]> = SmallVec::new();
        for op in &self.ops {
            match op {
                Op::Var(var) => stack.push(Cow::borrowed(var)),
                Op::And => {
                    let b = stack.pop().ok_or(fmt::Error)?;
                    let a = stack.pop().ok_or(fmt::Error)?;
                    stack.push(Cow::owned(format!("({} && {})", a, b)));
                }
                Op::Or => {
                    let b = stack.pop().ok_or(fmt::Error)?;
                    let a = stack.pop().ok_or(fmt::Error)?;
                    stack.push(Cow::owned(format!("({} || {})", a, b)));
                }
                Op::Not => {
                    let a = stack.pop().ok_or(fmt::Error)?;
                    stack.push(Cow::owned(format!("!({})", a)));
                }
            }
        }

        if stack.len() != 1 {
            return Err(fmt::Error);
        }

        let exp = stack.pop().ok_or(fmt::Error)?;
        write!(f, "{}", exp)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    #[test]
    fn to_string() {
        fn to_string(exp: &[Op]) -> String {
            let mut text = String::default();
            write!(text, "{}", Exp { ops: exp.into() }).expect("malformed expression");
            text
        }

        assert_eq!(to_string(&[Op::Var("a"), Op::Not]), "!(a)");
        assert_eq!(
            to_string(&[Op::Var("a"), Op::Var("b"), Op::And]),
            "(a && b)"
        );
        assert_eq!(
            to_string(&[Op::Var("a"), Op::Var("b"), Op::Or, Op::Not,]),
            "!((a || b))"
        );
        assert_eq!(
            to_string(&[
                Op::Var("a"),
                Op::Var("b"),
                Op::Var("c"),
                Op::And,
                Op::Or,
                Op::Not,
            ]),
            "!((a || (b && c)))"
        );
        assert_eq!(
            to_string(&[
                Op::Var("a"),
                Op::Var("b"),
                Op::Not,
                Op::Var("c"),
                Op::And,
                Op::Or,
            ]),
            "(a || (!(b) && c))"
        );
    }

    #[test]
    fn parse() {
        fn test(exp: &[Op]) {
            let input = Exp { ops: exp.into() };
            let mut text = String::default();
            write!(text, "{}", input).expect("malformed expression");
            assert_eq!(Exp::from_str(&text), input);
        }

        test(&[Op::Var("a"), Op::Not]);
        test(&[Op::Var("b"), Op::Var("a"), Op::And]);
        test(&[Op::Var("b"), Op::Var("a"), Op::Or, Op::Not]);
        test(&[
            Op::Var("c"),
            Op::Var("b"),
            Op::And,
            Op::Var("a"),
            Op::Or,
            Op::Not,
        ]);
        test(&[
            Op::Var("c"),
            Op::Var("b"),
            Op::Not,
            Op::And,
            Op::Var("a"),
            Op::Or,
        ]);

        fn to_string(exp: &str) -> String {
            let exp = Exp::from_str(exp);
            let mut text = String::default();
            write!(text, "{}", exp).expect("malformed expression");
            text
        }

        // test some degenerated combinations

        assert_eq!(to_string("!a"), "!(a)");
        assert_eq!(to_string(" ! a "), "!(a)");
        assert_eq!(to_string(" !\ta "), "!(a)");
        assert_eq!(to_string(" !\ta    "), "!(a)");
        assert_eq!(to_string(" !\ta  \t "), "!(a)");

        assert_eq!(to_string("|b&&&a"), "(|b && &a)");
        assert_eq!(to_string("b||a"), "(b || a)");
        assert_eq!(
            to_string("some_big$string@||!other_value023"),
            "(some_big$string@ || !(other_value023))"
        );
    }
}
