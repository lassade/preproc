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

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Ctx {
    pub vars: HashMap<String, bool>,
    stack: Vec<bool>,
}

#[inline(always)]
const unsafe fn str_from_raw_parts<'a>(ptr: *const u8, len: usize) -> &'a str {
    core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len))
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
    pub fn from_str(exp: &'a str) -> Result<Self, Error> {
        // uses the shunting yard algorithm
        // https://en.wikipedia.org/wiki/Shunting_yard_algorithm

        #[derive(PartialOrd, Ord, PartialEq, Eq, Clone, Copy)]
        enum Token {
            And = 0,
            Or = 1,
            Not = 2,
            Noop,
            LParen,
        }

        // translate a [`Token`] to a `Op` and precedence
        const OPERATORS: &'static [Op<'static>] = &[Op::And, Op::Or, Op::Not];
        const PRECEDENCE: &'static [usize] = &[0, 0, 1];

        let mut stack: SmallVec<[Token; 16]> = SmallVec::new();
        let mut ops = Vec::with_capacity(16);

        let data = exp.as_bytes();
        let mut ptr = data.as_ptr();
        let mut token_ptr = ptr;
        let ptr_end = unsafe { ptr.add(data.len()) };

        let break_ch = unsafe {
            _mm_set_epi8(
                0,
                0,
                0,
                0,
                0,
                0,
                b'\0' as i8,
                b'\r' as i8,
                b'\n' as i8,
                b'\t' as i8, // 6
                b' ' as i8,  // 5
                b'!' as i8,  // 4
                b'&' as i8,  // 3
                b'(' as i8,  // 2
                b')' as i8,  // 1
                b'|' as i8,  // 0
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

            // doesn't need to check for utf8 continuation bits, because they will be handled in the variable section

            let break_mask =
                unsafe { _mm_movemask_epi8(_mm_cmpeq_epi8(_mm_set1_epi8(ch as _), break_ch)) };

            if break_mask != 0 {
                if break_mask & 0b1111_1111_1110_0000 != 0 {
                    // accept and skip spaces,
                    token_ptr = ptr;
                    continue;
                }

                if break_mask & 0b0000_0100 != 0 {
                    token_ptr = ptr; // accept the token
                    stack.push(Token::LParen);
                    continue;
                }

                if break_mask & 0b0000_0010 != 0 {
                    token_ptr = ptr; // accept the token
                    loop {
                        if let Some(o) = stack.pop() {
                            if o != Token::LParen {
                                ops.push(unsafe { *OPERATORS.get_unchecked(o as usize) });
                            } else {
                                break;
                            }
                        } else {
                            return Err(Error {
                                offset: unsafe { ptr.offset_from(data.as_ptr()) } as usize - 1,
                                len: 1,
                                message: Cow::borrowed("unmached `)`"),
                            });
                        }
                    }
                    continue;
                }

                let op0;
                if break_mask & 0b0000_1000 != 0 {
                    // and
                    if ptr >= ptr_end || unsafe { *ptr } != b'&' {
                        return Err(Error {
                            offset: unsafe { ptr.offset_from(data.as_ptr()) } as usize - 1,
                            len: 1,
                            message: Cow::borrowed("expecting `&&`"),
                        });
                    }
                    ptr = unsafe { ptr.add(1) };
                    op0 = Token::And;
                } else if break_mask & 0b0000_0001 != 0 {
                    // or
                    if ptr >= ptr_end || unsafe { *ptr } != b'|' {
                        return Err(Error {
                            offset: unsafe { ptr.offset_from(data.as_ptr()) } as usize - 1,
                            len: 1,
                            message: Cow::borrowed("expecting `||`"),
                        });
                    }
                    ptr = unsafe { ptr.add(1) };
                    op0 = Token::Or;
                } else if break_mask & 0b0001_0000 != 0 {
                    // not
                    op0 = Token::Not;
                } else {
                    op0 = Token::Noop;
                }
                if op0 != Token::Noop {
                    token_ptr = ptr; // accept the token
                    loop {
                        let pre0 = unsafe { *PRECEDENCE.get_unchecked(op0 as usize) };
                        if let Some(&op1) = stack.last() {
                            if op1 == Token::LParen {
                                break;
                            }
                            let pre1 = unsafe { *PRECEDENCE.get_unchecked(op1 as usize) };
                            if pre0 <= pre1 {
                                ops.push(unsafe { *OPERATORS.get_unchecked(op1 as usize) });
                                stack.pop();
                                continue;
                            }
                        }
                        break;
                    }
                    stack.push(op0);
                    continue;
                }
            }

            // fast path for variable appending
            loop {
                if ptr >= ptr_end {
                    // accept the token
                } else {
                    // not very good vor short variable names
                    // ignore spaces
                    let break_mask = unsafe {
                        let chunk = _mm_loadu_si128(ptr as *const _); // 6 cycles
                        _mm_movemask_epi8(_mm_or_si128(
                            _mm_or_si128(
                                _mm_or_si128(
                                    _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b' ' as i8)),
                                    _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'\t' as i8)),
                                ),
                                _mm_or_si128(
                                    _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'!' as i8)),
                                    _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'&' as i8)),
                                ),
                            ),
                            _mm_or_si128(
                                _mm_or_si128(
                                    _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'(' as i8)),
                                    _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b')' as i8)),
                                ),
                                _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'|' as i8)),
                            ),
                        )) // 13 + 3 cycles
                    };
                    if break_mask != 0 {
                        // found something
                        let break_offset = break_mask.trailing_zeros() as usize;
                        if break_offset > 0 {
                            // out of bounds check
                            ptr = unsafe { ptr.add(break_offset) };
                            if ptr > ptr_end {
                                ptr = ptr_end;
                            }
                            // accept the token
                        }
                    } else {
                        ptr = unsafe { ptr.add(16) };
                        continue;
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

        while let Some(o) = stack.pop() {
            if o == Token::LParen {
                return Err(Error {
                    offset: 0, // todo: position offset
                    len: 0,
                    message: Cow::borrowed("unmached `(`"),
                });
            }
            ops.push(unsafe { *OPERATORS.get_unchecked(o as usize) });
        }

        Ok(Self { ops })
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
                Ok(val) => panic!(
                    "malformed expression `{}` was parsed as: `{}` {:?}",
                    exp, &val, &val.ops
                ),
                Err(_) => {}
            }
        }

        // unary check
        check("b && a !");
        check("b && a !c");
        check("b || a && c !");

        // missing operators
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
