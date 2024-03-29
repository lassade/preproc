//! Handle `#include`, `#if` and `#define` `#undef` directives in any source file

extern crate alloc;

use alloc::{boxed::Box, rc::Rc, string::String, vec, vec::Vec};
use core::fmt;

use hashbrown::{HashMap, HashSet};
use smartstring::{Compact, SmartString};

pub mod exp;
use exp::{Ctx, Exp};

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod sse2;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub use sse2::{parse_exp, parse_file};

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

// todo: a code block should reduce the ammount of memory to store it all
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
    // each line self referece str slices of `_data` that's why the lifelime is 'static
    lines: Vec<Line<'static>>,
}

impl File {
    pub fn parse(data: String, config: &Config) -> Self {
        let mut lines = vec![];

        // safety: `data` will live as long as each line because they are kept
        // inside the same struct inaccessible to the end user
        let borrow = unsafe { &*(&data as *const String) };
        sse2::parse_file(&borrow, config, |line| lines.push(line));

        Self { _data: data, lines }
    }
}

pub trait FileLoader {
    // todo: return a parsed File with the proper file path
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
    state: State,
    state_stack: Vec<State>,
    outputted_line_count: usize,
}

impl Default for PreProcessor {
    fn default() -> Self {
        Self {
            config: Config::default(),
            file_loader: Box::new(DefaultFileLoader::default()),
            files: HashMap::default(),
            defines: HashSet::with_capacity(32),
            ctx: Ctx::default(),
            state: State {
                value: true,
                value_flipped_by_else_block: true,
            },
            state_stack: Vec::with_capacity(4),
            outputted_line_count: 1,
        }
    }
}

impl PreProcessor {
    pub fn with_loader(file_loader: impl FileLoader + 'static) -> Self {
        Self {
            file_loader: Box::new(file_loader),
            ..Default::default()
        }
    }

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

    fn process_file(&mut self, file_path: &str, file: &File, f: &mut impl FnMut(&str)) {
        // does the acctual processing recursively

        let stack_depth = self.state_stack.len();

        for (line_count, line) in file.lines.iter().enumerate() {
            match line {
                Line::Code(line) | Line::Rem(line) => {
                    // default behaviour is to remove lines
                    if self.state.value {
                        (f)(line);
                        self.outputted_line_count += 1;
                    }
                }
                Line::Inc(inc) => {
                    // load and recursively add theses lines to the current one
                    if let Some(inc_file) = self.preload(inc) {
                        self.process_file(inc, inc_file.as_ref(), f);
                    } else {
                        panic!(
                            "couldn't find include file \"{}\" at {}:{}",
                            inc,
                            file_path,
                            line_count + 1
                        );
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
                    if self.state_stack.len() == 0 {
                        panic!(
                            "`elif` doesn't have a maching `if` at {}:{}",
                            file_path,
                            line_count + 1
                        );
                    }

                    if self.state.value_flipped_by_else_block {
                        panic!("`elif` after `else` at {}:{}", file_path, line_count + 1);
                    }

                    if !self.state.value {
                        // state still false, evel expression to see if will print the next lines of code
                        self.state.value = exp.eval(&mut self.ctx);
                    }
                }
                Line::Else => {
                    if self.state_stack.len() == 0 {
                        panic!(
                            "`else` doesn't have a maching `if` at {}:{}",
                            file_path,
                            line_count + 1
                        );
                    }

                    if self.state.value_flipped_by_else_block {
                        panic!("`else` after `else` at {}:{}", file_path, line_count + 1);
                    }

                    self.state.value = !self.state.value;
                    self.state.value_flipped_by_else_block = true;
                }
                Line::Endif => {
                    if let Some(prev_state) = self.state_stack.pop() {
                        self.state = prev_state;
                    } else {
                        panic!(
                            "`endif` doesn't have a maching `if` at {}:{}",
                            file_path,
                            line_count + 1
                        );
                    }
                }
            }
        }

        if stack_depth != self.state_stack.len() {
            panic!("some `if` block is open in file {}", file_path);
        }
    }

    pub fn process(&mut self, path: &str, mut f: impl FnMut(&str)) {
        if let Some(file) = self.preload(path) {
            // clear state
            self.ctx.clear();
            self.outputted_line_count = 0;
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
            self.process_file(path, file.as_ref(), &mut f);
        } else {
            panic!("file \"{}\" not found", path);
        }
    }

    pub fn process_to_str(&mut self, path: &str, string: &mut String) {
        self.process(path, |text| {
            string.push_str(text);
            string.push_str("\n");
        });
    }

    pub fn process_to_writer(&mut self, path: &str, mut writer: impl std::io::Write) {
        self.process(path, |text| {
            writeln!(writer, "{}", text).expect("failed to write line");
        });
    }

    fn find_defines_of_file(&mut self, file: &File, defines: &mut HashSet<SmartString<Compact>>) {
        for line in file.lines.iter() {
            match line {
                Line::Inc(inc) => {
                    // load and recursively add theses lines to the current one
                    if let Some(inc_file) = self.preload(inc) {
                        self.find_defines_of_file(inc_file.as_ref(), defines);
                    }
                }
                &Line::Def(def) => {
                    defines.insert(def.into());
                }
                Line::If(exp) | Line::Elif(exp) => {
                    for op in &exp.ops {
                        match op {
                            &exp::Op::Var(def) => {
                                defines.insert(def.into());
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub fn find_defines_of(&mut self, path: &str, defines: &mut HashSet<SmartString<Compact>>) {
        if let Some(file) = self.preload(path) {
            self.find_defines_of_file(file.as_ref(), defines);
            // remove constant defines
            defines.remove("0");
            defines.remove("1");
            defines.remove("true");
            defines.remove("false");
        } else {
            panic!("file \"{}\" not found", path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        const FILES: &[(&str, usize)] = &[
            ("benches/files/Native.g.cs", 177),
            ("benches/files/shader.wgsl", 636),
        ];

        let config = Config::default();

        for &(path, line_count) in FILES {
            let input = std::fs::read_to_string(path).expect("file not found");
            let file = File::parse(input, &config);
            assert_eq!(file.lines.len(), line_count);
        }
    }

    #[test]
    fn defines_of_file() {
        let mut pre_processor = PreProcessor {
            file_loader: Box::new({
                let mut file_loader = DefaultFileLoader::default();
                file_loader.search_paths.push("benches/files".into());
                file_loader
            }),
            ..Default::default()
        };

        let mut defines = HashSet::with_capacity(32);
        pre_processor.find_defines_of("main.c", &mut defines);

        for def in ["COMMON_HEADER", "OTHER_DEFINE"] {
            assert!(
                defines.contains(def),
                "define `{}` not found in {:?}",
                def,
                &defines
            );
        }
    }

    #[test]
    fn bevy() {
        let mut pre_processor = PreProcessor {
            file_loader: Box::new({
                let mut file_loader = DefaultFileLoader::default();
                file_loader.search_paths.push("benches/files/bevy".into());
                file_loader
            }),
            ..Default::default()
        };

        let mut defines = HashSet::with_capacity(32);
        pre_processor.find_defines_of("pbr/pbr.wgsl", &mut defines);

        for def in [
            "NO_ARRAY_TEXTURES_SUPPORT",
            "DIRECTIONAL_LIGHT_SHADOW_MAP_DEBUG_CASCADES",
            "PREMULTIPLY_ALPHA",
            "MOTION_VECTOR_PREPASS",
            "LOAD_PREPASS_NORMALS",
            "DEPTH_PREPASS",
            "TONEMAP_METHOD_REINHARD_LUMINANCE",
            "TONEMAP_METHOD_NONE",
            "ENVIRONMENT_MAP",
            "VERTEX_COLORS",
            "TONEMAP_METHOD_REINHARD",
            "TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM",
            "STANDARDMATERIAL_NORMAL_MAP",
            "BLEND_MULTIPLY",
            "NORMAL_PREPASS",
            "VERTEX_TANGENTS",
            "VERTEX_UVS",
            "TONEMAP_METHOD_TONY_MC_MAPFACE",
            "SKINNED",
            "CLUSTERED_FORWARD_DEBUG_Z_SLICES",
            "CLUSTERED_FORWARD_DEBUG_CLUSTER_LIGHT_COMPLEXITY",
            "SIXTEEN_BYTE_ALIGNMENT",
            "TONEMAP_METHOD_AGX",
            "TONEMAP_METHOD_ACES_FITTED",
            "TONEMAP_IN_SHADER",
            "CLUSTERED_FORWARD_DEBUG_CLUSTER_COHERENCY",
            "LIGHTS_USE_STORAGE",
            "PREPASS_FRAGMENT",
            "MULTISAMPLED",
            "DEBAND_DITHER",
            "BLEND_PREMULTIPLIED_ALPHA",
            "TONEMAP_METHOD_BLENDER_FILMIC",
        ] {
            assert!(defines.contains(def), "define `{}` not found", def,);
        }

        let mut output = String::with_capacity(32 * 1024 * 1024);
        pre_processor.process_to_str("pbr/pbr.wgsl", &mut output);

        assert_eq!(pre_processor.outputted_line_count, 1219);
    }
}
