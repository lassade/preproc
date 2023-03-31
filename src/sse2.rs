#![allow(unused)]

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

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

pub fn str_cmp(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }

    if a.is_empty() {
        return true;
    }

    let mut rem = a.len();
    let mut a = a.as_bytes().as_ptr();
    let mut b = b.as_bytes().as_ptr();

    loop {
        // safety: by checking if `a` and `b` is not empty is possible to ensure that neither is null
        let cmp_mask = unsafe {
            _mm_movemask_epi8(_mm_cmpeq_epi8(
                _mm_loadu_si128(a as *const _),
                _mm_loadu_si128(b as *const _),
            )) // 6 + 6 + 1 + 3  cycles
        };

        if rem < 17 {
            return (cmp_mask & MASK[rem]) == MASK[rem];
        }

        unsafe {
            a = a.add(16);
            b = b.add(16);
        }

        rem -= 16;
    }
}

#[inline(always)]
unsafe fn line<'a>(ptr: *const u8, mut ptr_end: *const u8) -> &'a str {
    // remove '\r' if any
    let prev = ptr_end.sub(1);
    if ptr <= prev && *prev == b'\r' {
        ptr_end = prev;
    }
    str_from_raw_parts(ptr, ptr_end.offset_from(ptr) as usize)
}

#[inline(always)]
unsafe fn next_space(chunk: __m128i) -> i32 {
    _mm_movemask_epi8(_mm_or_si128(
        _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b' ' as i8)), // 0x20 (32)
        _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'\t' as i8)), // 0x0B (11)
    )) // 8 cycles
}

#[inline(always)]
unsafe fn next_space_or_enter(chunk: __m128i) -> i32 {
    _mm_movemask_epi8(_mm_or_si128(
        _mm_or_si128(
            _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b' ' as i8)), // 0x20 (32)
            _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'\t' as i8)), // 0x0B (11)
        ),
        _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'\n' as i8)), // 0x0A (10)
    )) // 10 cycles
}

#[inline(always)]
unsafe fn next_enter(chunk: __m128i) -> i32 {
    _mm_movemask_epi8(
        _mm_cmpeq_epi8(chunk, _mm_set1_epi8(b'\n' as i8)), // 0x0A (10)
    ) // 4 cycles
}

#[inline(always)]
unsafe fn ignore_space(chunk: __m128i) -> i32 {
    !next_space(chunk)
}

pub fn parse_file<'a>(input: &'a str, config: &Config, mut f: impl FnMut(Line<'a>)) {
    if input.is_empty() {
        return;
    }

    let data = input.as_bytes();

    let mut ptr = data.as_ptr();
    let ptr_end = unsafe { ptr.add(data.len()) };

    // todo: keep track line count for proper error handling
    // todo: keep track the line begin
    let mut line_ptr = ptr;

    'main: loop {
        unsafe {
            if ptr >= ptr_end {
                return;
            }

            let chunk = _mm_loadu_si128(ptr as *const _); // 6 cycles
            let space_mask = ignore_space(chunk); // fixme: doesn't account for the fact that and space might alreadt
            if space_mask != 0 {
                // found something
                let space_offset = space_mask.trailing_zeros() as usize;

                // out of bounds check
                ptr = ptr.add(space_offset);
                if ptr >= ptr_end {
                    return;
                }

                let ch = *ptr;
                if ch == config.special_char {
                    // directive
                    ptr = ptr.add(1);
                    let mut directive_ptr = ptr;

                    loop {
                        if ptr >= ptr_end {
                            // empty directive
                            // todo: output as a code line
                            panic!("empty directive");
                            return;
                        }

                        let chunk = _mm_loadu_si128(ptr as *const _); // 6 cycles
                        let enter_mask = next_space_or_enter(chunk); // todo: support comments
                        if enter_mask != 0 {
                            // found something
                            let enter_offset = enter_mask.trailing_zeros() as usize;

                            // out of bounds check
                            ptr = ptr.add(enter_offset);
                            if ptr >= ptr_end {
                                // push directive
                                let name = line(directive_ptr, ptr_end);
                                (f)(parse_directive(name, None));
                                return;
                            }

                            let ch = *ptr;
                            if ch == b'\n' {
                                // push directive
                                let name = line(directive_ptr, ptr);
                                (f)(parse_directive(name, None));

                                ptr = ptr.add(1); // skip '\n'

                                line_ptr = ptr;

                                // execute the next loop break to the 'main loop
                                continue 'main;
                            } else {
                                // this directive might have an argument, cotinue to the next loop
                                break;
                            }
                        } else {
                            // keep going
                            ptr = ptr.add(16);
                        }
                    }

                    // save the directive name '#' dir_name [dir_arg] ['\n']
                    let name = str_from_range(directive_ptr, ptr);

                    ptr = ptr.add(1); // skip the space

                    // todo: SIMD might not be needed in where because isn't expected mutch more than a single space
                    // ignore empty spaces
                    loop {
                        if ptr >= ptr_end {
                            (f)(parse_directive(name, None));
                            return;
                        }

                        if *ptr == b' ' || *ptr == b'\t' {
                            ptr = ptr.add(1);
                        } else {
                            break;
                        }
                    }

                    directive_ptr = ptr;

                    // the frist time the next loop executes the ptr should be whitin the range
                    debug_assert!(ptr < ptr_end);

                    // directive argument
                    loop {
                        let chunk = _mm_loadu_si128(ptr as *const _); // 6 cycles

                        // todo: support comments

                        let enter_mask = next_enter(chunk);
                        if enter_mask != 0 {
                            // found something
                            let enter_offset = enter_mask.trailing_zeros() as usize;

                            // out of bounds check
                            ptr = ptr.add(enter_offset);
                            if ptr >= ptr_end {
                                ptr = ptr_end;
                            }

                            // push directive with argument
                            let arg = line(directive_ptr, ptr);
                            (f)(parse_directive(name, Some(arg)));

                            ptr = ptr.add(1); // skip '\n'

                            line_ptr = ptr;
                            break;
                        } else {
                            // keep going
                            ptr = ptr.add(16);

                            if ptr >= ptr_end {
                                // push the directive
                                let arg = str_from_range(directive_ptr, ptr_end);
                                (f)(parse_directive(name, Some(arg)));
                                return;
                            }
                        }
                    }
                } else if ch == b'\n' {
                    // empty line
                    (f)(Line::Code(str_from_raw_parts(line_ptr, 0)));

                    // skip '\n'
                    ptr = ptr.add(1);

                    line_ptr = ptr;
                } else {
                    // line
                    loop {
                        if ptr >= ptr_end {
                            (f)(Line::Code(line(line_ptr, ptr_end)));
                            return;
                        }

                        let chunk = _mm_loadu_si128(ptr as *const _); // 6 cycles
                        let enter_mask = next_enter(chunk);
                        if enter_mask != 0 {
                            // found something
                            let enter_offset = enter_mask.trailing_zeros() as usize;

                            // out of bounds check
                            ptr = ptr.add(enter_offset);
                            if ptr >= ptr_end {
                                (f)(Line::Code(line(line_ptr, ptr_end)));
                                return;
                            }

                            (f)(Line::Code(line(line_ptr, ptr)));

                            // skip '\n'
                            ptr = ptr.add(1);

                            line_ptr = ptr;
                            break;
                        } else {
                            // keep going
                            ptr = ptr.add(16);
                        }
                    }
                }
            } else {
                // keep going
                ptr = ptr.add(16);
            }
        }
    }
}

fn parse_directive<'a>(name: &'a str, arg: Option<&'a str>) -> Line<'a> {
    #[inline(always)]
    fn cmp(a: __m128i, a_len: usize, b: &str) -> bool {
        if a_len != b.len() {
            return false;
        }

        // safety: by checking if `a` and `b` is not empty is possible to ensure that neither is null
        let cmp_mask = unsafe {
            _mm_movemask_epi8(_mm_cmpeq_epi8(a, _mm_loadu_si128(b.as_ptr() as *const _)))
            // 6 + 1 + 3  cycles
        };

        return (cmp_mask & MASK[b.len()]) == MASK[b.len()];
    }

    if name.is_empty() {
        panic!("empty directive");
    }

    let a = unsafe { _mm_loadu_si128(name.as_ptr() as *const _) }; // 6 cycles
    if cmp(a, name.len(), "if") {
        Line::If(Exp::from_str(arg.expect("missing `if` expression")).unwrap())
    } else if cmp(a, name.len(), "elif") {
        Line::Elif(Exp::from_str(arg.expect("missing `elif` expression")).unwrap())
    } else if cmp(a, name.len(), "else") {
        Line::Else
    } else if cmp(a, name.len(), "endif") {
        Line::Endif
    } else if cmp(a, name.len(), "include") {
        Line::Inc(arg.expect("missing `include` directive"))
    } else if cmp(a, name.len(), "define") {
        Line::Def(arg.expect("missing `define` directive"))
    } else if cmp(a, name.len(), "undef") {
        Line::Undef(arg.expect("missing `define` directive"))
    } else {
        // todo: unsupported directives should be writen as a line of code
        panic!("unsupported directive `{}`", name)
    }
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
}
