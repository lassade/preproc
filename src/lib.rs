//! Handle `#include`, `#if` and `#define` `#undef` directives in any source file

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

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
pub mod exp;
mod sse2;
