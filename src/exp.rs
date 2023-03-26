#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

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
    /// Operators tokens, mathc the order of [`OPERATORS`](Self::OPERATORS)
    pub const TOKENS: &'static [&'static str] = &["&&", "||", "!"];

    /// Supported operators, the slice order gives the precedence
    pub const OPERATORS: &'static [Op<'static>] = &[Op::And, Op::Or, Op::Not];
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

// /// Break chars, must be sorted
// const BREAK: &'static [u8] = &[b'\t', b' ', b'!', b'&', b'(', b')', b'|'];

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

        let mut stack: SmallVec<[Option<Op<'a>>; 8]> = SmallVec::new();
        let mut ops = vec![];

        let data = exp.as_bytes();
        let mut ptr = data.as_ptr();
        let mut token_ptr = ptr;
        let ptr_end = unsafe { ptr.add(data.len()) };

        let break_ch = unsafe {
            _mm_set_epi8(
                b'\t' as i8,
                b' ' as i8,
                b'!' as i8,
                b'&' as i8,
                b'(' as i8,
                b')' as i8,
                b'|' as i8,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            )
        };

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

            if ch == b' ' || ch == b'\t' {
                // accept and skip spaces
                token_ptr = ptr;
                continue;
            }

            if ch == b'(' {
                token_ptr = ptr; // accept the token
                stack.push(None);
                continue;
            }

            if ch == b')' {
                token_ptr = ptr; // accept the token
                while let Some(val) = stack.last().copied() {
                    if val.is_none() {
                        stack.pop();
                        break;
                    } else {
                        ops.push(stack.pop().unwrap().unwrap());
                    }
                }
                continue;
            }

            // safety: str slice respect the utf8 chars continuation bytes, because it will only split in ascii chars
            let token =
                unsafe { str_from_raw_parts(token_ptr, ptr.offset_from(token_ptr) as usize) };

            if token.len() == 1 {
                if ch == b'&' || ch == b'|' {
                    // just don't accept these as tokens
                    continue;
                }
            }

            if let Some(i) = Op::TOKENS.iter().position(|&r| r == token) {
                token_ptr = ptr; // accept the token
                loop {
                    if let Some(Some(op)) = stack.last() {
                        // a bit faster than looking into the `Op::OPERATORS` array
                        let j = match op {
                            Op::And => 0,
                            Op::Or => 1,
                            Op::Not => 2,
                            _ => break,
                        };
                        if i <= j {
                            ops.push(*op);
                            stack.pop();
                            continue;
                        }
                    }
                    break;
                }
                stack.push(Some(unsafe { *Op::OPERATORS.get_unchecked(i) }));
                continue;
            }

            // fast path for variable appending
            loop {
                if ptr >= ptr_end {
                    // accept the token
                } else {
                    // fetch the next char
                    // safety: ptr is within `str`, bounds
                    let ch;
                    unsafe {
                        ch = *ptr;
                        ptr = ptr.add(1);
                    }

                    // only process ascii chars
                    if ch > 127 {
                        continue;
                    }

                    unsafe {
                        if (_mm_movemask_epi8(_mm_cmpeq_epi8(_mm_set1_epi8(ch as _), break_ch))
                            & 0b1111_1110_0000_0000)
                            == 0
                        {
                            // continue appending more chars
                            continue;
                        }
                    }

                    // // note: binary search is slow here
                    // if BREAK.iter().position(|&r| r == ch).is_none() {
                    //     // continue appending more chars
                    //     continue;
                    // }

                    // todo: kinda dumb
                    unsafe {
                        ptr = ptr.sub(1);
                    }
                }

                // safety: str slice respect the utf8 chars continuation bytes, because it will only split in ascii chars
                let token =
                    unsafe { str_from_raw_parts(token_ptr, ptr.offset_from(token_ptr) as usize) };
                ops.push(Op::Var(token));

                token_ptr = ptr; // accept the token
                break;
            }
        }

        while let Some(token) = stack.pop() {
            if let Some(token) = token {
                ops.push(token);
            }
        }

        Self { ops }
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
