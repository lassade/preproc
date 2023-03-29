#![allow(unused)]

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

use crate::{
    exp::{Exp, Op},
    Line, Val,
};

#[derive(Default)]
pub struct File<'a> {
    lines: Vec<Line<'a>>,
}

#[inline(always)]
const unsafe fn str_from_raw_parts<'a>(ptr: *const u8, len: usize) -> &'a str {
    core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len))
}

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

fn parse<'a>(input: &'a str) -> File<'a> {
    let mut file = File::default();

    if input.is_empty() {
        return file;
    }

    let data = input.as_bytes();

    let mut ptr = data.as_ptr();
    let ptr_end = unsafe { ptr.add(data.len()) };

    // todo: keep track the line begin
    let mut line_ptr = ptr;

    loop {
        unsafe {
            if ptr >= ptr_end {
                file.lines.push(Line::EOF);
                return file;
            }

            let chunk = _mm_loadu_si128(ptr as *const _); // 6 cycles
            let space_mask = ignore_space(chunk); // fixme: doesn't account for the fact that and space might alreadt
            if space_mask != 0 {
                // found something
                let space_offset = space_mask.trailing_zeros() as usize;

                // out of bounds check
                ptr = ptr.add(space_offset);
                if ptr >= ptr_end {
                    file.lines.push(Line::EOF);
                    return file;
                }

                // todo: multiline comments

                let ch = *ptr;
                // todo: configurable directive char must be a ascii char, like '#' or '@' or '%'
                if ch == b'#' {
                    // directive
                    ptr = ptr.add(1);
                    let directive_ptr = ptr;

                    loop {
                        if ptr >= ptr_end {
                            // todo: empty directive ???
                            // todo: end of directive

                            file.lines.push(Line::EOF);
                            return file;
                        }

                        let chunk = _mm_loadu_si128(ptr as *const _); // 6 cycles
                        let enter_mask = next_space_or_enter(chunk);
                        if enter_mask != 0 {
                            // found something
                            let enter_offset = enter_mask.trailing_zeros() as usize;

                            // out of bounds check
                            ptr = ptr.add(enter_offset);
                            if ptr >= ptr_end {
                                // todo: end of line

                                file.lines.push(Line::EOF);
                                return file;
                            }

                            let ch = *ptr;
                            if ch == b'\n' {
                                // todo: if ch is a b'\n' don't execute the next loop break to the 'main loop
                                // todo: end directive

                                ptr = ptr.add(1); // skip the newline
                                line_ptr = ptr;
                                break;
                            } else {
                                todo!();
                            }
                        } else {
                            // keep going
                            ptr = ptr.add(16);
                        }
                    }

                    // // directive argument
                    // loop {
                    //     if ptr >= ptr_end {
                    //         // todo: end of directive

                    //         file.lines.push(Line::EOF);
                    //         return file;
                    //     }

                    //     let chunk = _mm_loadu_si128(ptr as *const _); // 6 cycles
                    //     let enter_mask = next_enter(chunk);
                    //     if enter_mask != 0 {
                    //         // found something
                    //         let enter_offset = enter_mask.trailing_zeros() as usize;

                    //         // out of bounds check
                    //         ptr = ptr.add(enter_offset);
                    //         if ptr >= ptr_end {
                    //             // todo: end of line

                    //             file.lines.push(Line::EOF);
                    //             return file;
                    //         }

                    //         // todo: end directive

                    //         ptr = ptr.add(2); // skip the newline
                    //         line_ptr = ptr;
                    //         break;
                    //     } else {
                    //         // keep going
                    //         ptr = ptr.add(16);
                    //     }
                    // }
                } else if ch == b'\n' {
                    // empty line
                    file.lines.push(Line::Code(str_from_raw_parts(line_ptr, 0)));
                    ptr = ptr.add(1);
                    line_ptr = ptr;
                } else {
                    // line
                    loop {
                        if ptr >= ptr_end {
                            file.lines.push(Line::Code(str_from_raw_parts(
                                line_ptr,
                                ptr_end.offset_from(line_ptr) as _,
                            )));
                            file.lines.push(Line::EOF);
                            return file;
                        }

                        let chunk = _mm_loadu_si128(ptr as *const _); // 6 cycles
                        let enter_mask = next_enter(chunk);
                        if enter_mask != 0 {
                            // found something
                            let enter_offset = enter_mask.trailing_zeros() as usize;

                            // out of bounds check
                            ptr = ptr.add(enter_offset);
                            if ptr >= ptr_end {
                                file.lines.push(Line::Code(str_from_raw_parts(
                                    line_ptr,
                                    ptr_end.offset_from(line_ptr) as _,
                                )));
                                file.lines.push(Line::EOF);
                                return file;
                            }

                            file.lines.push(Line::Code(str_from_raw_parts(
                                line_ptr,
                                ptr.offset_from(line_ptr) as _,
                            )));

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

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    fn test(lines: &[Line]) {
        let mut text = String::default();

        for (i, line) in lines.iter().enumerate() {
            match line {
                Line::Code(code) => {
                    text.push_str(code);
                }
                Line::Directive(d, exp) => {
                    text.push('#');
                    text.push_str(d);
                    if let Some(exp) = exp {
                        text.push(' ');
                        write!(text, "{}", exp).expect("invalid directive expression");
                    }
                }
                Line::EOF => break,
            }

            if i < lines.len() - 1 {
                text.push('\n');
            }
        }

        assert_eq!(parse(&text).lines, lines, "{}", &text);
    }

    #[test]
    fn no_directives() {
        test(&[
            Line::Code("// some comment"),
            Line::Code(""),
            Line::Code("fn func() -> f32 {"),
            Line::Code("    return 1.0;"),
            Line::Code("}"),
            Line::EOF,
        ]);
        test(&[
            Line::Code("// some comment\r"),
            Line::Code("\r"),
            Line::Code("fn func() -> f32 {\r"),
            Line::Code("    return 1.0;\r"),
            Line::Code("}\r"),
            Line::EOF,
        ]);
    }

    // #[test]
    // fn ifelse() {
    //     test(&[
    //         Line::Code("// some comment"),
    //         Line::Code(""),
    //         Line::Code("fn func() -> f32 {"),
    //         Line::Directive("    #if SHADOWS", vec![]),
    //         Line::Code("    return 0.0;"),
    //         Line::Directive("    #else", vec![]),
    //         Line::Code("    return 1.0;"),
    //         Line::Directive("    #endif", vec![]),
    //         Line::Code("}"),
    //         Line::EOF,
    //     ]);
    // }
}
