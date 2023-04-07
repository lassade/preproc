use alloc::{format, vec::Vec};
use core::fmt;

use beef::Cow;
use hashbrown::HashSet;
use smallvec::SmallVec;
use smartstring::{Compact, SmartString};

/// Type aliasing for a [`SmartString`] using the [`Compact`] mode of aggressive inlining
pub type String = SmartString<Compact>;

/// Operands and Operators
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Op<'a> {
    Var(&'a str),
    And,
    Or,
    Not,
    // todo: Xor
}

#[derive(Debug, PartialEq, Eq)]
pub struct Ctx {
    pub vars: HashSet<String>,
    stack: Vec<bool>,
}

impl Default for Ctx {
    fn default() -> Self {
        let mut vars = HashSet::with_capacity(16);
        vars.insert("true".into());
        vars.insert("1".into());
        Self {
            vars,
            stack: Vec::with_capacity(8),
        }
    }
}

impl Ctx {
    pub fn clear(&mut self) {
        self.vars.clear();
        self.vars.insert("true".into());
        self.vars.insert("1".into());
    }
}

#[derive(Debug)]
pub struct Error {
    pub offset: usize,
    pub len: usize,
    pub message: Cow<'static, str>,
}

/// Expression, internally it uses the Reverse Polish Notation (RPN) notation
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Exp<'a> {
    pub ops: Vec<Op<'a>>,
}

impl<'a> Exp<'a> {
    #[inline]
    pub fn from_str(exp: &'a str) -> Result<Self, Error> {
        crate::parse_exp(exp)
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
                Op::Var(var) => ctx.stack.push(ctx.vars.contains(*var)),
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
            assert_eq!(
                Exp::from_str(&text).expect("failed to parse expression"),
                input
            );
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

        // testing utf8 support

        test(&[
            Op::Var("منزل"),
            Op::Var("دجاجة"),
            Op::Not,
            Op::And,
            Op::Var("جرو"),
            Op::Or,
        ]);
        test(&[
            Op::Var("猴"),
            Op::Var("小狗"),
            Op::Not,
            Op::And,
            Op::Var("房子"),
            Op::Or,
        ]);
        test(&[
            Op::Var("Будинок"),
            Op::Var("щеня"),
            Op::Not,
            Op::And,
            Op::Var("клавіатура"),
            Op::Or,
        ]);

        fn to_string(exp: &str) -> String {
            let exp = Exp::from_str(exp).expect("failed to parse expression");
            let mut text = String::default();
            write!(text, "{}", exp).expect("malformed expression");
            text
        }

        assert_eq!(to_string("b && !a"), "(b && !(a))");
        assert_eq!(to_string("!b && a"), "(!(b) && a)");
        assert_eq!(to_string("!b && !a"), "(!(b) && !(a))");
        assert_eq!(to_string("!b && !a || c"), "((!(b) && !(a)) || c)");

        // test some degenerated combinations

        assert_eq!(to_string("!a"), "!(a)");
        assert_eq!(to_string(" ! a "), "!(a)");
        assert_eq!(to_string(" !\ta "), "!(a)");
        assert_eq!(to_string(" !\ta    "), "!(a)");
        assert_eq!(to_string(" !\ta  \t "), "!(a)");

        // unary operators can be applyied in both left and right
        assert_eq!(to_string("b && a !"), "(b && !(a))");
        assert_eq!(to_string("(b && a)!"), "!((b && a))");
        assert_eq!(to_string("b || a && c !"), "((b || a) && !(c))");

        assert_eq!(to_string("b||a"), "(b || a)");
        assert_eq!(
            to_string("some_big$string@||!other_value023"),
            "(some_big$string@ || !(other_value023))"
        );
    }

    #[test]
    fn malformed() {
        fn check(exp: &str) {
            match Exp::from_str(exp) {
                Ok(val) => {
                    if val.is_valid() {
                        panic!(
                            "expression `{}` was parsed as: `{}` {:?}",
                            exp, &val, &val.ops
                        );
                    } else {
                        panic!(
                            "expression `{}` was parsed as an invalid `Exp`: {:?}",
                            exp, &val.ops
                        );
                    }
                }
                Err(_) => {}
            }
        }

        // missing operators
        check("b && a !c ||");
        check("b a && c ! ||");
        check("b && a !c");
        check("||a");
        check("&&a");
        check("b || a &&");
        check("b || a ||");

        check("b & a");
        check("b | a");
        check("b || a &");
        check("b && a |");
        check("|b&&&a");
        check("b&&&a");

        check("((b&&a)");
        check("((b&&a)))");
        check("((b&&(c||a))))");
        check("((b&(c||a))))");
    }
}
