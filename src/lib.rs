//! Handle `#include`, `#if` and `#define` `#undef` directives in any source file

extern crate alloc;

use alloc::{boxed::Box, rc::Rc, string::String, vec, vec::Vec};
use core::fmt;

use hashbrown::{HashMap, HashSet};
use smartstring::{Compact, SmartString};

mod chars;
pub mod exp;
mod sse2;

use exp::{Ctx, Exp};

// exports
pub use sse2::parse_file;

pub struct Config {
    /// Special ASCII character used to define the start of an directive, default is `b'#'`
    /// but is possible to configure to something like `b'@'`, `b'%'` or `b'!'`
    pub special_char: u8,
    /// Single line comment string, default "//"
    pub comment: SmartString<Compact>,
    // multiline comments are just too problematic
    // /// Start of a multi-line comment, default "/*"
    // pub comment_begin: SmartString<Compact>,
    // /// End of a multi-line comment, default "*/"
    // pub comment_end: SmartString<Compact>,
    /// Start of a include path, default `b'\"'`
    pub include_begin: u8,
    /// Delimiter the end of a include path, default "`b'\"'`, make sure to use a ASCII that
    /// isn't included in the path it self like `b'>' for instance
    pub include_end: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            special_char: b'#',
            comment: "//".into(),
            // comment_begin: "/*".into(),
            // comment_end: "*/".into(),
            include_begin: b'\"',
            include_end: b'\"',
        }
    }
}

#[inline(always)]
const unsafe fn str_from_raw_parts<'a>(ptr: *const u8, len: usize) -> &'a str {
    core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len))
}

#[inline(always)]
const unsafe fn str_from_range<'a>(ptr: *const u8, ptr_end: *const u8) -> &'a str {
    str_from_raw_parts(ptr, ptr_end.offset_from(ptr) as usize)
}

#[derive(Debug, PartialEq, Eq)]
pub enum Line<'a> {
    Code(&'a str),
    Rem(&'a str),
    Inc(&'a str),
    Def(&'a str),
    Undef(&'a str),
    If(Exp<'a>),
    Elif(Exp<'a>),
    Else,
    Endif,
}

impl<'a> fmt::Display for Line<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Line::Code(line) | Line::Rem(line) => write!(f, "{}", line),
            Line::Inc(path) => write!(f, "#include \"{}\"", path),
            Line::Def(def) => write!(f, "#define {}", def),
            Line::Undef(def) => write!(f, "#undef {}", def),
            Line::If(exp) => write!(f, "#if {}", exp),
            Line::Elif(exp) => write!(f, "#elif {}", exp),
            Line::Else => write!(f, "#else"),
            Line::Endif => write!(f, "#endif"),
        }
    }
}

pub struct File {
    _data: String,
    // lines self referece str inside `_data` that's why the lifelime is static
    lines: Vec<Line<'static>>,
}

impl File {
    pub fn parse(data: String, config: &Config) -> Self {
        let mut lines = vec![];

        // safety: `data` will live as long as each line because they are kept
        // inside the same read-only struct
        let borrow = unsafe { &*(&data as *const String) };
        sse2::parse_file(&borrow, config, |line| lines.push(line));

        Self { _data: data, lines }
    }
}

pub trait FileLoader {
    fn load(&self, path: &str) -> Option<String>;
}

pub struct DefaultFileLoader {
    pub search_paths: Vec<String>,
}

impl Default for DefaultFileLoader {
    fn default() -> Self {
        let mut search_paths = vec![];
        // if let Ok(current_dir) = std::env::current_dir() {
        //     search_paths.push(current_dir.to_string_lossy().to_string());
        // }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                search_paths.push(exe_dir.to_string_lossy().to_string());
            }
        }
        Self { search_paths }
    }
}

impl FileLoader for DefaultFileLoader {
    fn load(&self, path: &str) -> Option<String> {
        use simdutf8::basic::from_utf8;

        if let Ok(data) = std::fs::read(path) {
            if let Err(err) = from_utf8(&data) {
                panic!("not a valid UTF-8 file, {}", err);
            }
            // safety: just checked using the from_utf8 function above
            return Some(unsafe { String::from_utf8_unchecked(data) });
        }

        let mut search_path = String::new();
        for base_path in &self.search_paths {
            search_path.clear();
            search_path.push_str(base_path);
            search_path.push_str(std::path::MAIN_SEPARATOR_STR);
            search_path.push_str(path);

            if let Ok(data) = std::fs::read(&search_path) {
                if let Err(err) = from_utf8(&data) {
                    panic!("not a valid UTF-8 file, {}", err);
                }
                // safety: just checked using the from_utf8 function above
                return Some(unsafe { String::from_utf8_unchecked(data) });
            }
        }

        None
    }
}

pub struct NoFileLoader;

impl FileLoader for NoFileLoader {
    fn load(&self, _: &str) -> Option<String> {
        None
    }
}

#[derive(Clone, Copy)]
struct State {
    // file_path: ...
    // line: usize,
    value: bool,
    value_flipped_by_else_block: bool,
}

pub struct PreProcessor {
    pub config: Config,
    pub file_loader: Box<dyn FileLoader>,
    pub files: HashMap<String, Rc<File>>,
    pub defines: HashSet<SmartString<Compact>>,
    ctx: Ctx,
    line_count: usize,
    state: State,
    state_stack: Vec<State>,
}

impl Default for PreProcessor {
    fn default() -> Self {
        Self {
            config: Config::default(),
            file_loader: Box::new(DefaultFileLoader::default()),
            files: HashMap::default(),
            defines: HashSet::default(),
            ctx: Ctx::default(),
            line_count: 1,
            state: State {
                value: true,
                value_flipped_by_else_block: true,
            },
            state_stack: Vec::with_capacity(4),
        }
    }
}

impl PreProcessor {
    pub fn preload(&mut self, path: &str) -> Option<Rc<File>> {
        match self.files.entry(path.into()) {
            hashbrown::hash_map::Entry::Occupied(ref entry) => Some(entry.get().clone()),
            hashbrown::hash_map::Entry::Vacant(entry) => self.file_loader.load(path).map(|data| {
                entry
                    .insert(Rc::new(File::parse(data, &self.config)))
                    .clone()
            }),
        }
    }

    fn process_file(&mut self, file: &File, mut f: impl FnMut(&str)) {
        // does the acctual processing recursively

        for line in &file.lines {
            match line {
                Line::Code(line) | Line::Rem(line) => {
                    // default behaviour is to remove lines
                    if self.state.value {
                        if self.line_count > 1 {
                            (f)("\n");
                        }

                        (f)(line);

                        self.line_count += 1;
                    }
                }
                Line::Inc(inc) => {
                    // load and recursively add theses lines to the current one
                    if let Some(inc_file) = self.preload(inc) {
                        self.process_file(inc_file.as_ref(), &mut f);
                    } else {
                        panic!("couldn't find include file \"{}\"", inc);
                    }
                }
                &Line::Def(def) => {
                    if self.ctx.vars.insert(def.into()) {
                        // todo: warn about defining the same variable twice
                    }
                }
                &Line::Undef(def) => {
                    if !self.ctx.vars.remove(def) {
                        // todo: warn about undefining a variable that isn't defined
                    }
                }
                Line::If(exp) => {
                    self.state_stack.push(self.state);
                    self.state.value = exp.eval(&mut self.ctx);
                    self.state.value_flipped_by_else_block = false;
                }
                Line::Elif(exp) => {
                    let state = self
                        .state_stack
                        .last_mut()
                        .expect("`elif` doesn't have a maching `if`");

                    if state.value_flipped_by_else_block {
                        panic!("`elif` after `else`");
                    }

                    if !state.value {
                        // state still false, try out to see if
                        state.value = exp.eval(&mut self.ctx);
                    }
                }
                Line::Else => {
                    let state = self
                        .state_stack
                        .last_mut()
                        .expect("`else` doesn't have a maching `if`");

                    if state.value_flipped_by_else_block {
                        panic!("`else` after `else`");
                    }

                    state.value = !state.value;
                    state.value_flipped_by_else_block = true;
                }
                Line::Endif => {
                    self.state = self
                        .state_stack
                        .pop()
                        .expect("`endif` doesn't have a maching `if`");
                }
            }
        }

        assert!(!self.state_stack.is_empty(), "`if` block is open");
    }

    pub fn process(&mut self, path: &str, f: impl FnMut(&str)) {
        if let Some(file) = self.preload(path) {
            // clear state
            self.ctx.clear();
            self.line_count = 0;
            self.state = State {
                value: true,
                value_flipped_by_else_block: true,
            };
            self.state_stack.clear();

            // include user defines
            for def in &self.defines {
                self.ctx.vars.insert(def.clone());
            }

            // begin processing files
            self.process_file(file.as_ref(), f);
        } else {
            panic!("file \"{}\" not found", path);
        }
    }

    pub fn process_to_writer(&mut self, path: &str, mut writer: impl std::io::Write) {
        self.process(path, |text| {
            write!(writer, "{}", text).expect("...");
        });
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    const FILES: &[&str] = &["benches/files/Native.g.cs", "benches/files/shader.wgsl"];

    #[test]
    fn basic() {
        let config = Config::default();

        for path in FILES {
            let input = std::fs::read_to_string(path).expect("file not found");
            //File::from_str(&input, &config);

            // used to create the source of truth
            let mut output =
                std::fs::File::create(format!("{}.t", path)).expect("failed to create output file");
            sse2::parse_file(&input, &config, |line| {
                writeln!(output, "{:?}", &line).unwrap();
            });
        }
    }
}
