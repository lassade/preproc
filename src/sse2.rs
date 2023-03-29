#![allow(unused)]

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

use crate::{
    exp::{Exp, Op},
    str_from_raw_parts, File, Line, Val,
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

pub struct Config<'a> {
    /// Special ASCII character used to define the start of and directive, default is `b'#'`
    /// but is possible to configure to something like `b'@'`, `b'%'` or `b'!'`
    pub special_char: u8,
    /// Single line comment string, default "//"
    pub comment: &'a str,
    // /// Start of a multi-line comment, default "/*"
    // pub comment_begin: &'a str,
    // /// End of a multi-line comment, default "*/"
    // pub comment_end: &'a str,
}

impl<'a> Default for Config<'a> {
    fn default() -> Self {
        Self {
            special_char: b'#',
            comment: "//",
        }
    }
}

fn parse<'a>(input: &'a str, config: &Config) -> File<'a> {
    let mut file = File::default();

    if input.is_empty() {
        return file;
    }

    let data = input.as_bytes();

    let mut ptr = data.as_ptr();
    let ptr_end = unsafe { ptr.add(data.len()) };

    // todo: keep track the line begin
    let mut line_ptr = ptr;

    'main: loop {
        unsafe {
            if ptr >= ptr_end {
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
                    return file;
                }

                // todo: multiline comments

                let ch = *ptr;
                if ch == config.special_char {
                    // directive
                    ptr = ptr.add(1);
                    let mut dir_ptr = ptr;

                    loop {
                        if ptr >= ptr_end {
                            // todo: empty directive ???
                            // todo: end of directive

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
                                ptr = ptr_end;

                                // push directive
                                let dir_name =
                                    str_from_raw_parts(dir_ptr, ptr.offset_from(dir_ptr) as usize);
                                file.lines.push(Line::Directive(dir_name, None));

                                return file;
                            }

                            let ch = *ptr;
                            if ch == b'\n' {
                                // push directive
                                let dir_name =
                                    str_from_raw_parts(dir_ptr, ptr.offset_from(dir_ptr) as usize);
                                file.lines.push(Line::Directive(dir_name, None));

                                ptr = ptr.add(1); // skip the newline
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
                    let dir_name = str_from_raw_parts(dir_ptr, ptr.offset_from(dir_ptr) as usize);

                    ptr = ptr.add(1); // skip the space

                    // todo: simd might not be needed in where because isn't expected mutch more than a single space
                    // ignore empty spaces
                    loop {
                        if ptr >= ptr_end {
                            file.lines.push(Line::Directive(dir_name, None));
                            return file;
                        }

                        if *ptr == b' ' || *ptr == b'\t' {
                            ptr = ptr.add(1);
                        } else {
                            break;
                        }
                    }

                    dir_ptr = ptr;

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
                            let dir_arg =
                                str_from_raw_parts(dir_ptr, ptr.offset_from(dir_ptr) as usize);
                            file.lines
                                .push(Line::Directive(dir_name, Some(Val::Raw(dir_arg))));

                            ptr = ptr.add(1); // skip the newline
                            line_ptr = ptr;
                            break;
                        } else {
                            // keep going
                            ptr = ptr.add(16);

                            if ptr >= ptr_end {
                                ptr = ptr_end;

                                // push the directive
                                let dir_arg =
                                    str_from_raw_parts(dir_ptr, ptr.offset_from(dir_ptr) as usize);
                                file.lines
                                    .push(Line::Directive(dir_name, Some(Val::Raw(dir_arg))));
                                return file;
                            }
                        }
                    }
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
            }

            if i < lines.len() - 1 {
                text.push('\n');
            }
        }

        let config = Config::default();

        assert_eq!(parse(&text, &config).lines, lines, "{}", &text);
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
        test(&[
            Line::Code("// some comment\r"),
            Line::Code("\r"),
            Line::Code("fn func() -> f32 {\r"),
            Line::Code("    return 1.0;\r"),
            Line::Code("}\r"),
        ]);
    }

    #[test]
    fn ifelse() {
        test(&[
            Line::Code("// some comment"),
            Line::Code(""),
            Line::Code("fn func() -> f32 {"),
            Line::Directive("if", Some(Val::Raw("SHADOWS"))),
            Line::Code("    return 0.0;"),
            Line::Directive("else", None),
            Line::Code("    return 1.0;"),
            Line::Directive("endif", None),
            Line::Code("}"),
        ]);
    }
}
