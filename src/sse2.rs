#![allow(unused)]

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;
use core::ptr::null;

use smallvec::SmallVec;

use crate::{
    exp::{Exp, Op},
    str_from_range, str_from_raw_parts, Config, Line,
};

const MASK: [i32; 17] = {
    let mut index = 0;
    let mut arr = [0; 17];
    loop {
        if index >= arr.len() {
            break;
        }
        arr[index] = ((1 << index) - 1) as i32;
        index += 1;
    }
    arr
};

#[inline(always)]
unsafe fn line<'a>(ptr: *const u8, mut ptr_end: *const u8) -> &'a str {
    // todo: bake inside the Parser::enter fn
    // remove '\r' if any
    let prev = ptr_end.sub(1);
    if ptr <= prev && *prev == b'\r' {
        ptr_end = prev;
    }
    str_from_raw_parts(ptr, ptr_end.offset_from(ptr) as usize)
}

// safety: `alen` and `b.len()` must be up to 16 characters long
#[inline(always)]
unsafe fn start_with(a: __m128i, alen: usize, b: &[u8]) -> bool {
    if alen < b.len() {
        // not enough characters
        return false;
    }

    let cmp_mask = _mm_movemask_epi8(_mm_cmpeq_epi8(a, _mm_loadu_si128(b.as_ptr() as *const _))); // 6 + 1 + 3  cycles
    return (cmp_mask & MASK[b.len()]) == MASK[b.len()];
}

struct Parser {
    ptr: *const u8,
    ptr_end: *const u8,
    line_count: usize,
    line_ptr: *const u8,
}

impl Parser {
    fn new() -> Self {
        Self {
            ptr: null(),
            ptr_end: null(),
            line_count: 0,
            line_ptr: null(),
        }
    }

    #[inline(always)]
    unsafe fn mask_and_find(&mut self, f: impl Fn(__m128i) -> i32) -> bool {
        while self.ptr < self.ptr_end {
            let chunk = _mm_loadu_si128(self.ptr as *const _); // 6 cycles
            let mask = (f)(chunk); // 8 cycles
            if mask != 0 {
                // found something
                let offset = mask.trailing_zeros() as usize;

                // out of bounds check
                self.ptr = self.ptr.add(offset);

                return true;
            } else {
                self.ptr = self.ptr.add(16);
            }
        }

        false
    }

    unsafe fn ignore_space(&mut self) -> bool {
        self.mask_and_find(|chunk| {
            !_mm_movemask_epi8(_mm_or_si128(
                _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b' ' as i8)), // 0x20 (32)
                _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'\t' as i8)), // 0x0B (11)
            )) // 8 cycles
        })
    }

    unsafe fn find(&mut self, ch: u8) -> bool {
        self.mask_and_find(|chunk| {
            _mm_movemask_epi8(_mm_cmpeq_epi8(chunk, _mm_set1_epi8(ch as i8))) // 4 cycles
        })
    }

    unsafe fn find_enter_or(&mut self, ch: u8) -> bool {
        self.mask_and_find(|chunk| {
            !_mm_movemask_epi8(_mm_or_si128(
                _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'\n' as i8)), // 0x0A (10)
                _mm_cmpeq_epi8(chunk, _mm_set1_epi8(ch as i8)),
            )) // 8 cycles
        })
    }

    unsafe fn find_space_or_enter(&mut self) -> bool {
        self.mask_and_find(|chunk| {
            _mm_movemask_epi8(_mm_or_si128(
                _mm_or_si128(
                    _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b' ' as i8)), // 0x20 (32)
                    _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'\t' as i8)), // 0x0B (11)
                ),
                _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'\n' as i8)), // 0x0A (10)
            )) // 10 cycles
        })
    }

    #[inline(always)]
    fn enter(&mut self) {
        self.line_count += 1;
        self.line_ptr = self.ptr;
    }

    #[inline(always)]
    unsafe fn char_pos(&self) -> usize {
        str_from_range(self.line_ptr, self.ptr).chars().count()
    }

    unsafe fn exp<'a, 'b>(&mut self, config: &'b Config) -> Exp<'a> {
        // copied from exp.rs, but modified to support comments and newline
        //
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
        const OPERATORS: &[Op<'static>] = &[Op::And, Op::Or, Op::Not];
        const PRECEDENCE: &[usize] = &[0, 0, 1];

        let mut stack: SmallVec<[(Token, *const u8); 16]> = SmallVec::new();
        let mut ops = Vec::with_capacity(16);

        let comment_char = config
            .comment
            .as_bytes()
            .get(0)
            .copied()
            .unwrap_or_default() as i8;
        let comment_rem = config.comment.as_bytes().get(1..).unwrap_or_default();

        let mut token_ptr = self.ptr;

        let break_ch = _mm_set_epi8(
            0,
            0,
            0,
            0,
            0,
            comment_char, // 10
            b'\0' as i8,  // 9
            b'\r' as i8,  // 8
            b'\n' as i8,  // 7
            b'\t' as i8,  // 6
            b' ' as i8,   // 5
            b'!' as i8,   // 4
            b'&' as i8,   // 3
            b'(' as i8,   // 2
            b')' as i8,   // 1
            b'|' as i8,   // 0
        );

        loop {
            if self.ptr >= self.ptr_end {
                break;
            }

            let ch = *self.ptr;
            self.ptr = self.ptr.add(1);

            // doesn't need to check for utf8 continuation bits, because they will be handled in the variable section

            let break_mask =
                unsafe { _mm_movemask_epi8(_mm_cmpeq_epi8(_mm_set1_epi8(ch as _), break_ch)) };

            if break_mask != 0 {
                if break_mask & 0b1111_1011_0110_0000 != 0 {
                    // accept and skip
                    token_ptr = self.ptr;
                    continue;
                }

                if break_mask & 0b1000_0000 != 0 {
                    // enter, roll back and break
                    self.ptr = self.ptr.sub(1);
                    break;
                }

                if break_mask & 0b0000_0100_0000_0000 != 0 {
                    // check if is a comment
                    // todo: usually just less than 4 chars, maybe just use a default `str::starts_with`
                    let chunk = _mm_loadu_si128(self.ptr as *const _); // 6 cycles
                    let len = self.ptr_end.offset_from(self.ptr) as usize;
                    if start_with(chunk, len, comment_rem) {
                        // roll back and break
                        self.ptr = self.ptr.sub(1);
                        break;
                    }
                }

                if break_mask & 0b0000_0100 != 0 {
                    token_ptr = self.ptr; // accept the token
                    stack.push((Token::LParen, self.ptr));
                    continue;
                }

                if break_mask & 0b0000_0010 != 0 {
                    token_ptr = self.ptr; // accept the token
                    loop {
                        if let Some((token, _)) = stack.pop() {
                            if token != Token::LParen {
                                ops.push(unsafe { *OPERATORS.get_unchecked(token as usize) });
                            } else {
                                break;
                            }
                        } else {
                            panic!("unmached `)` {}:{}", self.line_count, self.char_pos());
                        }
                    }
                    continue;
                }

                let op0;
                if break_mask & 0b0000_1000 != 0 {
                    // and
                    if self.ptr >= self.ptr_end || unsafe { *self.ptr } != b'&' {
                        panic!("expecting `&&` {}:{}", self.line_count, self.char_pos());
                    }
                    self.ptr = self.ptr.add(1);
                    op0 = Token::And;
                } else if break_mask & 0b0000_0001 != 0 {
                    // or
                    if self.ptr >= self.ptr_end || unsafe { *self.ptr } != b'|' {
                        panic!("expecting `||` {}:{}", self.line_count, self.char_pos());
                    }
                    self.ptr = self.ptr.add(1);
                    op0 = Token::Or;
                } else if break_mask & 0b0001_0000 != 0 {
                    // not
                    op0 = Token::Not;
                } else {
                    op0 = Token::Noop;
                }
                if op0 != Token::Noop {
                    token_ptr = self.ptr; // accept the token
                    loop {
                        let pre0 = unsafe { *PRECEDENCE.get_unchecked(op0 as usize) };
                        if let Some(&(op1, _)) = stack.last() {
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
                    stack.push((op0, self.ptr));
                    continue;
                }
            }

            // fast path for variable appending
            loop {
                if self.ptr >= self.ptr_end {
                    // accept the token and limit the ptr
                    self.ptr = self.ptr_end;
                } else {
                    // not very good vor short variable names
                    // ignore spaces
                    let break_mask = unsafe {
                        let chunk = _mm_loadu_si128(self.ptr as *const __m128i); // 6 cycles
                        _mm_movemask_epi8(_mm_or_si128(
                            _mm_or_si128(
                                _mm_or_si128(
                                    _mm_or_si128(
                                        _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'\r' as i8)),
                                        _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'\n' as i8)),
                                    ),
                                    _mm_or_si128(
                                        _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b' ' as i8)),
                                        _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'\t' as i8)),
                                    ),
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
                                _mm_or_si128(
                                    _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'|' as i8)),
                                    _mm_cmpeq_epi8(chunk, _mm_set1_epi8(comment_char)),
                                ),
                            ),
                        )) // 19 + 3 cycles
                    };
                    if break_mask != 0 {
                        // found something
                        let break_offset = break_mask.trailing_zeros() as usize;
                        if break_offset > 0 {
                            // out of bounds check
                            self.ptr = self.ptr.add(break_offset);
                            if self.ptr > self.ptr_end {
                                self.ptr = self.ptr_end;
                            }
                            // accept the token
                        }
                    } else {
                        self.ptr = self.ptr.add(16);
                        continue;
                    }
                }

                // safety: str slice respect the utf8 chars continuation bytes, because it will only split in ascii chars
                let token = unsafe { str_from_range(token_ptr, self.ptr) };
                ops.push(Op::Var(token));

                token_ptr = self.ptr; // accept the token
                break;
            }
        }

        while let Some((token, offset)) = stack.pop() {
            if token == Token::LParen {
                panic!("unmached `(` {}:{}", self.line_count, self.char_pos());
            }
            ops.push(unsafe { *OPERATORS.get_unchecked(token as usize) });
        }

        // todo: check if the expression is valid or not

        Exp { ops }
    }

    unsafe fn parse<'a, 'b>(
        &mut self,
        data: &'a str,
        config: &'b Config,
        mut f: impl FnMut(Line<'a>),
    ) {
        // make some assertions about the lenght of the comments
        assert!(
            config.comment.len() <= 16,
            "`comment` \"{}\" exceeded 16 chars limit",
            config.comment
        );

        self.ptr = data.as_ptr();
        self.ptr_end = self.ptr.add(data.len());

        self.line_count = 1;
        self.line_ptr = self.ptr;

        while self.ptr < self.ptr_end {
            if !self.ignore_space() {
                // nothing left
                break;
            }

            let ch = *self.ptr;

            if ch == b'\n' {
                // empty line, notice that the line pointer is inportant
                (f)(Line::Code(str_from_raw_parts(self.line_ptr, 0)));

                // consume '\n'
                self.ptr = self.ptr.add(1);

                self.enter();

                continue;
            }

            if ch == config.special_char {
                // directive
                self.ptr = self.ptr.add(1);

                let len = self.ptr_end.offset_from(self.ptr) as usize;
                if len != 0 {
                    let chunk = _mm_loadu_si128(self.ptr as *const _); // 6 cycles

                    if start_with(chunk, len, b"if") {
                        self.ptr = self.ptr.add(b"if".len());
                        (f)(Line::If(self.exp(config)));
                    } else if start_with(chunk, len, b"elif") {
                        self.ptr = self.ptr.add(b"elif".len());
                        (f)(Line::Elif(self.exp(config)));
                    } else if start_with(chunk, len, b"else") {
                        self.ptr = self.ptr.add(b"else".len());
                        (f)(Line::Else);
                    } else if start_with(chunk, len, b"endif") {
                        self.ptr = self.ptr.add(b"endif".len());
                        (f)(Line::Endif);
                    } else if start_with(chunk, len, b"undef") {
                        self.ptr = self.ptr.add(b"undef".len());

                        // todo: should "undef  \n" case be handled?
                        self.ignore_space();

                        let def_ptr = self.ptr;

                        // todo: usually just less than 4 chars, maybe just use a default `str::starts_with`
                        let chunk = _mm_loadu_si128(self.ptr as *const _); // 6 cycles
                        let len = self.ptr_end.offset_from(self.ptr) as usize;
                        if start_with(chunk, len, config.comment.as_bytes()) {
                            panic!(
                                "missing define name of `define` {}:{}",
                                self.line_count,
                                self.char_pos()
                            );
                        }

                        if !self.find_space_or_enter() {
                            self.ptr = self.ptr_end;
                        }

                        (f)(Line::Undef(str_from_range(def_ptr, self.ptr)));
                    } else if start_with(chunk, len, b"define") {
                        self.ptr = self.ptr.add(b"define".len());

                        // todo: should "undef  \n" case be handled?
                        self.ignore_space();

                        let def_ptr = self.ptr;

                        // todo: usually just less than 4 chars, maybe just use a default `str::starts_with`
                        let chunk = _mm_loadu_si128(self.ptr as *const _); // 6 cycles
                        let len = self.ptr_end.offset_from(self.ptr) as usize;
                        if start_with(chunk, len, config.comment.as_bytes()) {
                            panic!(
                                "missing define name of `define` {}:{}",
                                self.line_count,
                                self.char_pos()
                            );
                        }

                        if !self.find_space_or_enter() {
                            self.ptr = self.ptr_end;
                        }

                        (f)(Line::Def(str_from_range(def_ptr, self.ptr)));
                    } else if start_with(chunk, len, b"include") {
                        self.ptr = self.ptr.add(b"include".len());

                        self.ignore_space();

                        // assert the char is '\"'
                        if self.ptr >= self.ptr_end || *self.ptr != config.include_begin {
                            panic!(
                                "missing start delimiter '{:?}' of `include` {}:{}",
                                char::from_u32_unchecked(config.include_begin as _),
                                self.line_count,
                                self.char_pos()
                            );
                        }

                        // consume delimiter
                        self.ptr = self.ptr.add(1);

                        let inc_ptr = self.ptr;

                        // consume chars until find a \n or a \"

                        if !self.find(config.include_end) {
                            // assert the char is \"
                            panic!(
                                "missing end delimiter '{:?}' of `include` {}:{}",
                                char::from_u32_unchecked(config.include_begin as _),
                                self.line_count,
                                self.char_pos()
                            );
                        }

                        // send a event
                        (f)(Line::Inc(line(inc_ptr, self.ptr)));

                        // consume delimiter
                        self.ptr = self.ptr.add(1);
                    } else {
                        // unknown directives will be treated as lines of code
                        if !self.find(b'\n') {
                            // return the remaning of the the data without going out of bounds
                            self.ptr = self.ptr_end
                        }

                        (f)(Line::Code(line(self.line_ptr, self.ptr)));

                        // skip '\n'
                        self.ptr = self.ptr.add(1);

                        self.enter();

                        continue;
                    }

                    if self.ptr >= self.ptr_end {
                        break;
                    }
                }

                // account for "\r\n" line end format, this is important to avoid output extra `Line::Rem` events
                if *self.ptr == b'\r' {
                    self.ptr = self.ptr.add(1);
                    if self.ptr >= self.ptr_end {
                        break;
                    }
                }

                if *self.ptr != b'\n' {
                    // remaning of the line if any will be treaded as a remaning of a line of code,
                    // unsupported directives also are threaded this way

                    let rem_ptr = self.ptr;

                    if !self.find(b'\n') {
                        // return the remaning of the the data without going out of bounds
                        self.ptr = self.ptr_end;
                    }

                    (f)(Line::Rem(line(rem_ptr, self.ptr)));
                }

                // consume '\n'
                self.ptr = self.ptr.add(1);

                self.enter();

                continue;
            }

            if !self.find(b'\n') {
                // return the remaning of the the data without going out of bounds
                self.ptr = self.ptr_end
            }

            (f)(Line::Code(line(self.line_ptr, self.ptr)));

            // skip '\n'
            self.ptr = self.ptr.add(1);

            self.enter();
        }
    }
}

pub fn parse_file<'a>(input: &'a str, config: &Config, mut f: impl FnMut(Line<'a>)) {
    let mut parser = Parser::new();
    unsafe { parser.parse(input, config, f) };
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    fn test(lines: &[Line]) {
        let mut text = String::default();

        for (i, line) in lines.iter().enumerate() {
            write!(text, "{}", line);
            if i < lines.len() - 1 {
                text.push('\n');
            }
        }

        let config = Config::default();
        let mut parsed_lines = vec![];
        parse_file(&text, &config, |line| parsed_lines.push(line));

        assert_eq!(parsed_lines, lines, "{}", &text);
    }

    #[test]
    fn no_directives() {
        test(&[
            Line::Code("// some comment"),
            Line::Code(""),
            Line::Code("fn func() -> f32 {"),
            Line::Code("    return 1.0;"),
            Line::Code("}"),
        ]);
    }

    #[test]
    fn inc() {
        test(&[
            Line::Inc("other_fn_header.wgsl"),
            Line::Code("// some comment"),
            Line::Code(""),
            Line::Code("fn func() -> f32 {"),
            Line::Code("    return other_fn(0.0);"),
            Line::Code("}"),
        ]);
    }

    #[test]
    fn ifelse() {
        test(&[
            Line::Code("// some comment"),
            Line::Code(""),
            Line::Code("fn func() -> f32 {"),
            Line::If(Exp::from_str("SHADOWS").unwrap()),
            Line::Code("    return 0.0;"),
            Line::Else,
            Line::Code("    return 1.0;"),
            Line::Endif,
            Line::Code("}"),
        ]);
    }

    #[test]
    fn degenerated() {
        // fn func() -> f32 {
        // #define // blank define
        //     /* some comment
        //     #if SHADOWS */ // should be readed as a line of code
        //     #if NOT_SHADOWS // line rem
        //     return 0.0;"),
        //     #else
        //     return 1.0;"),
        //     #endif
        // }
        todo!();
    }
}
