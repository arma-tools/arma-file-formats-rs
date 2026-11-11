//! Preprocessor for text configs (`config.cpp`, `.hpp`, `.ext`, `.rvmat`, `mission.sqm`, ...).
//!
//! Implements the Real Virtuality preprocessor: `#define` (object like, with arguments, variadic),
//! `#undef`, `#if`, `#ifdef`, `#ifndef`, `#else`, `#endif`, `#include`, the `#` and `##`
//! operators and the `__XXX__` built-in macros.
//!
//! Arma specifics that differ from the C preprocessor:
//! - macro arguments are expanded before they are substituted, also for `#` and `##`, so
//!   `#define QUOTE(x) #x` turns `QUOTE(FOO)` into the stringified expansion of `FOO`
//! - double-quoted strings are never expanded, single-quoted strings are
//! - only parentheses group commas in macro arguments, `[]` and `{}` do not
//! - a `#` in a macro body that is not followed by an argument name is dropped
//!
//! ```
//! use arma_file_formats::real_virtuality::rap::{Cfg, FsIncludeResolver, Preprocessor};
//!
//! let mut pp = Preprocessor::new().with_resolver(FsIncludeResolver::new().with_include_dir("P:\\"));
//! pp.define("DEBUG", "1").unwrap();
//! let cfg = Cfg::parse_config_with("#define VAL 3\nvalue = VAL;", None, &mut pp).unwrap();
//! assert_eq!(cfg.get_entry(&["value"]).unwrap().as_long(), Some(3));
//! ```

use std::{
    collections::{HashMap, VecDeque},
    fs, io,
    path::{Path, PathBuf},
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::errors::{PreprocessError, PreprocessErrorKind};

/// Includes nested deeper than this fail, which also stops include cycles.
pub const MAX_INCLUDE_DEPTH: usize = 32;

/// A file returned by an [`IncludeResolver`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeFile {
    /// Path of the file, passed back as `from` for includes inside it and used for `__FILE__`.
    pub path: String,
    pub content: String,
}

/// Looks up the files named by `#include` and `__has_include`.
///
/// Closures `Fn(&str, Option<&str>) -> io::Result<Option<IncludeFile>>` implement it, which
/// makes it easy to serve includes from a PBO or a map, see [`join_include_path`].
pub trait IncludeResolver {
    /// `path` is the path as written in the source, without the quotes. `from` is the path
    /// of the including file, `None` for a source that was passed in without a path.
    /// Returns `Ok(None)` when the file does not exist.
    fn resolve(&self, path: &str, from: Option<&str>) -> io::Result<Option<IncludeFile>>;
}

impl<F> IncludeResolver for F
where
    F: Fn(&str, Option<&str>) -> io::Result<Option<IncludeFile>>,
{
    fn resolve(&self, path: &str, from: Option<&str>) -> io::Result<Option<IncludeFile>> {
        self(path, from)
    }
}

/// Resolves includes on the file system.
///
/// Relative paths are looked up next to the including file, absolute paths (`\a3\...`) in the
/// include directories (for example the P drive). Relative paths fall back to the include
/// directories, and drive paths (`D:\file.hpp`) are opened as they are.
#[derive(Debug, Clone, Default)]
pub struct FsIncludeResolver {
    include_dirs: Vec<PathBuf>,
}

impl FsIncludeResolver {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_include_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.include_dirs.push(dir.into());
        self
    }

    pub fn add_include_dir(&mut self, dir: impl Into<PathBuf>) {
        self.include_dirs.push(dir.into());
    }

    fn candidates(&self, path: &str, from: Option<&str>) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        // A path of this system, `/tmp/config.cpp` on Linux, before the game's `\a3\...`
        if Path::new(path).is_absolute() {
            candidates.push(PathBuf::from(path));
        }
        let path = path.replace('\\', "/");
        if is_drive_path(&path) {
            return vec![PathBuf::from(path)];
        }
        let rel = path.trim_start_matches('/');
        if !path.starts_with('/') {
            match from {
                Some(from) => {
                    if let Some(parent) = Path::new(from).parent() {
                        candidates.push(parent.join(rel));
                    }
                }
                None => candidates.push(PathBuf::from(rel)),
            }
        }
        candidates.extend(self.include_dirs.iter().map(|dir| dir.join(rel)));
        candidates
    }
}

impl IncludeResolver for FsIncludeResolver {
    fn resolve(&self, path: &str, from: Option<&str>) -> io::Result<Option<IncludeFile>> {
        for candidate in self.candidates(path, from) {
            if candidate.is_file() {
                let content = String::from_utf8(fs::read(&candidate)?)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
                return Ok(Some(IncludeFile {
                    path: candidate.to_string_lossy().into_owned(),
                    content,
                }));
            }
        }
        Ok(None)
    }
}

const fn is_drive_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Resolves an include path against the path of the including file, like the game does.
///
/// Works in the game's virtual file system: absolute paths start with `\`, relative ones are
/// relative to the directory of `from`, `..` moves up a directory.
///
/// `join_include_path("..\\common.hpp", Some("\\x\\mod\\addons\\main\\config.cpp"))`
/// gives `\x\mod\addons\common.hpp`.
#[must_use]
pub fn join_include_path(path: &str, from: Option<&str>) -> String {
    let path = path.replace('/', "\\");
    if is_drive_path(&path) {
        return normalize_virtual(&path, false);
    }
    if path.starts_with('\\') {
        return normalize_virtual(&path, true);
    }
    from.map_or_else(
        || normalize_virtual(&path, false),
        |from| {
            let from = from.replace('/', "\\");
            let dir = from.rfind('\\').map_or("", |i| &from[..i]);
            normalize_virtual(&format!("{dir}\\{path}"), from.starts_with('\\'))
        },
    )
}

fn normalize_virtual(path: &str, absolute: bool) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('\\') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    let joined = parts.join("\\");
    if absolute {
        format!("\\{joined}")
    } else {
        joined
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Ident,
    /// Double-quoted string, including the quotes
    Str,
    Space,
    Newline,
    Hash,
    HashHash,
    /// Numbers, punctuation and anything else
    Other,
}

#[derive(Debug, Clone)]
struct Tok {
    kind: Kind,
    text: String,
}

impl Tok {
    fn new(kind: Kind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
        }
    }

    fn is(&self, text: &str) -> bool {
        self.kind == Kind::Other && self.text == text
    }
}

fn tokenize(src: &str) -> Vec<Tok> {
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut toks = Vec::with_capacity(len / 3);
    let mut i = 0;
    while i < len {
        let start = i;
        let kind = match bytes[i] {
            b'"' => {
                i = string_end(bytes, i);
                Kind::Str
            }
            b'\n' => {
                i += 1;
                Kind::Newline
            }
            b' ' | b'\t' | b'\r' => {
                while i < len && matches!(bytes[i], b' ' | b'\t' | b'\r') {
                    i += 1;
                }
                Kind::Space
            }
            b'#' => {
                if bytes.get(i + 1) == Some(&b'#') {
                    i += 2;
                    Kind::HashHash
                } else {
                    i += 1;
                    Kind::Hash
                }
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                Kind::Ident
            }
            c if c.is_ascii_digit() => {
                // Numbers swallow trailing letters so `1e5` is not split into `1` and ident `e5`
                while i < len
                    && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.')
                {
                    i += 1;
                }
                Kind::Other
            }
            _ => {
                i += src[i..].chars().next().map_or(1, char::len_utf8);
                Kind::Other
            }
        };
        toks.push(Tok::new(kind, &src[start..i]));
    }
    toks
}

/// End of the string starting at `start`. `""` inside a string is an escaped quote,
/// unterminated strings end at the line end.
fn string_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    loop {
        match bytes[i..].iter().position(|&b| b == b'"' || b == b'\n') {
            Some(p) if bytes[i + p] == b'"' => {
                i += p + 1;
                if bytes.get(i) == Some(&b'"') {
                    i += 1;
                } else {
                    return i;
                }
            }
            Some(p) => return i + p,
            None => return bytes.len(),
        }
    }
}

/// Removes `//` and `/* */` comments outside of double-quoted strings. Block comments keep
/// their line breaks so line numbers stay put.
fn strip_comments(src: &str) -> String {
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len);
    let mut last = 0;
    let mut i = 0;
    while i < len {
        match bytes[i] {
            b'"' => i = string_end(bytes, i),
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                out.push_str(&src[last..i]);
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                last = i;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                out.push_str(&src[last..i]);
                let end = src[i + 2..].find("*/").map_or(len, |p| i + 2 + p + 2);
                let newlines = src[i..end].matches('\n').count();
                if newlines == 0 {
                    out.push(' ');
                }
                out.extend(std::iter::repeat_n('\n', newlines));
                i = end;
                last = i;
            }
            _ => i += 1,
        }
    }
    out.push_str(&src[last..]);
    out
}

fn trim_spaces(mut toks: Vec<Tok>) -> Vec<Tok> {
    while toks
        .last()
        .is_some_and(|t| matches!(t.kind, Kind::Space | Kind::Newline))
    {
        toks.pop();
    }
    let start = toks
        .iter()
        .position(|t| !matches!(t.kind, Kind::Space | Kind::Newline))
        .unwrap_or(toks.len());
    toks.drain(..start);
    toks
}

fn join(toks: &[Tok]) -> String {
    toks.iter().map(|t| t.text.as_str()).collect()
}

/// Splits the tokens between balanced parentheses at top-level commas. `toks[open]` is the `(`.
/// Returns the arguments and the index after the closing `)`.
fn split_args(toks: &[Tok], open: usize) -> Option<(Vec<Vec<Tok>>, usize)> {
    let mut args = Vec::new();
    let mut cur = Vec::new();
    let mut depth = 0usize;
    for (i, t) in toks.iter().enumerate().skip(open + 1) {
        if t.is("(") {
            depth += 1;
        } else if t.is(")") {
            if depth == 0 {
                args.push(trim_spaces(cur));
                return Some((args, i + 1));
            }
            depth -= 1;
        } else if t.is(",") && depth == 0 {
            args.push(trim_spaces(std::mem::take(&mut cur)));
            continue;
        }
        cur.push(t.clone());
    }
    None
}

fn next_non_space(toks: &[Tok], from: usize) -> Option<usize> {
    (from..toks.len()).find(|&i| toks[i].kind != Kind::Space)
}

#[derive(Debug)]
struct Macro {
    /// `None` for object-like macros. Does not contain the `...` of variadic macros.
    params: Option<Vec<String>>,
    variadic: bool,
    body: Vec<Tok>,
}

impl Macro {
    fn param(&self, tok: &Tok) -> Option<usize> {
        if tok.kind != Kind::Ident {
            return None;
        }
        self.params.as_ref()?.iter().position(|p| *p == tok.text)
    }
}

struct Ctx<'a> {
    file: Option<&'a str>,
    line: usize,
}

#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
struct Cond {
    parent_active: bool,
    taken: bool,
    active: bool,
    seen_else: bool,
    line: usize,
}

/// The config preprocessor.
///
/// Macros and the `__COUNTER__` value persist between calls of [`Self::preprocess`], so use a
/// new preprocessor for each independent file. `__ARMA__` and `__ARMA3__` are predefined;
/// game dependent macros such as `__GAME_VER_MAJ__` or `__A3_DEBUG__` can be added with
/// [`Self::define`].
pub struct Preprocessor<'r> {
    macros: HashMap<String, Rc<Macro>>,
    resolver: Option<Box<dyn IncludeResolver + 'r>>,
    counter: u64,
    depth: usize,
    timestamp: Option<u64>,
}

impl Default for Preprocessor<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Preprocessor<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Preprocessor")
            .field("macros", &self.macros.keys().collect::<Vec<_>>())
            .field("has_resolver", &self.resolver.is_some())
            .field("counter", &self.counter)
            .finish_non_exhaustive()
    }
}

const BUILTINS: &[&str] = &[
    "__LINE__",
    "__FILE__",
    "__FILE_NAME__",
    "__FILE_SHORT__",
    "__COUNTER__",
    "__COUNTER_RESET__",
    "__DATE_ARR__",
    "__DATE_STR__",
    "__DATE_STR_ISO8601__",
    "__TIME__",
    "__TIME_UTC__",
    "__DAY__",
    "__MONTH__",
    "__YEAR__",
    "__TIMESTAMP_UTC__",
    "__RAND_INT8__",
    "__RAND_INT16__",
    "__RAND_INT32__",
    "__RAND_INT64__",
    "__RAND_UINT8__",
    "__RAND_UINT16__",
    "__RAND_UINT32__",
    "__RAND_UINT64__",
];

impl<'r> Preprocessor<'r> {
    /// A preprocessor without include resolver, `#include` fails with
    /// [`PreprocessErrorKind::NoIncludeResolver`] and `__has_include` is always 0.
    #[must_use]
    pub fn new() -> Self {
        let mut pp = Self {
            macros: HashMap::new(),
            resolver: None,
            counter: 0,
            depth: 0,
            timestamp: None,
        };
        for name in ["__ARMA__", "__ARMA3__"] {
            pp.macros.insert(
                name.to_owned(),
                Rc::new(Macro {
                    params: None,
                    variadic: false,
                    body: vec![Tok::new(Kind::Other, "1")],
                }),
            );
        }
        pp
    }

    #[must_use]
    pub fn with_resolver(mut self, resolver: impl IncludeResolver + 'r) -> Self {
        self.resolver = Some(Box::new(resolver));
        self
    }

    pub fn set_resolver(&mut self, resolver: impl IncludeResolver + 'r) {
        self.resolver = Some(Box::new(resolver));
    }

    /// Fixes the time used by the date and time macros (`__DATE_ARR__`, `__TIMESTAMP_UTC__`,
    /// ...) to a unix timestamp, `None` uses the current time. The game uses local time for
    /// some of them, this always uses UTC.
    pub const fn set_timestamp(&mut self, unix_seconds: Option<u64>) {
        self.timestamp = unix_seconds;
    }

    /// Defines a macro like `#define` does. `name` may have arguments: `define("ADD(a,b)", "a+b")`.
    pub fn define(&mut self, name: &str, body: &str) -> Result<(), PreprocessError> {
        self.define_directive(&format!("{name} {body}"))
            .map_err(|kind| PreprocessError {
                file: None,
                line: 0,
                kind,
            })
    }

    pub fn undefine(&mut self, name: &str) {
        self.macros.remove(name);
    }

    #[must_use]
    pub fn is_defined(&self, name: &str) -> bool {
        self.macros.contains_key(name) || BUILTINS.contains(&name)
    }

    /// Preprocesses `src`. `file` is its path, used to resolve relative includes and for
    /// `__FILE__` and error messages.
    ///
    /// Line breaks are kept where possible so that lines of the output match the source,
    /// until the first `#include` or multi-line macro.
    pub fn preprocess(&mut self, src: &str, file: Option<&str>) -> Result<String, PreprocessError> {
        self.run(src, file)
    }

    /// Loads `path` through the include resolver and preprocesses it.
    pub fn preprocess_path(&mut self, path: &str) -> Result<String, PreprocessError> {
        let err = |kind| PreprocessError {
            file: None,
            line: 0,
            kind,
        };
        let resolver = self
            .resolver
            .as_ref()
            .ok_or_else(|| err(PreprocessErrorKind::NoIncludeResolver(path.to_owned())))?;
        let file = resolver
            .resolve(path, None)
            .map_err(|source| {
                err(PreprocessErrorKind::IncludeIo {
                    path: path.to_owned(),
                    source,
                })
            })?
            .ok_or_else(|| err(PreprocessErrorKind::IncludeNotFound(path.to_owned())))?;
        self.run(&file.content, Some(&file.path))
    }

    fn run(&mut self, src: &str, file: Option<&str>) -> Result<String, PreprocessError> {
        let stripped = strip_comments(src);
        let lines: Vec<&str> = stripped.split('\n').collect();
        let mut out = String::with_capacity(stripped.len());
        let mut conds: Vec<Cond> = Vec::new();
        let mut pending = String::new();
        let mut pending_line = 1;

        let mut idx = 0;
        while idx < lines.len() {
            let line = lines[idx];
            if let Some(rest) = line.trim_start().strip_prefix('#') {
                let mut directive = rest.to_owned();
                let mut consumed = 1;
                while directive.trim_end().ends_with('\\') && idx + consumed < lines.len() {
                    let len = directive.trim_end().len() - 1;
                    directive.truncate(len);
                    directive.push(' ');
                    directive.push_str(lines[idx + consumed]);
                    consumed += 1;
                }

                self.flush(&mut pending, pending_line, file, &mut out)?;
                self.directive(&directive, idx + 1, file, &mut conds, &mut out)?;
                for k in 0..consumed {
                    if idx + k + 1 < lines.len() {
                        out.push('\n');
                    }
                }
                idx += consumed;
                pending_line = idx + 1;
                continue;
            }

            if conds.last().is_none_or(|c| c.active) {
                pending.push_str(line);
            }
            if idx + 1 < lines.len() {
                pending.push('\n');
            }
            idx += 1;
        }
        self.flush(&mut pending, pending_line, file, &mut out)?;

        if let Some(cond) = conds.last() {
            return Err(PreprocessError {
                file: file.map(str::to_owned),
                line: cond.line,
                kind: PreprocessErrorKind::UnterminatedIf,
            });
        }
        Ok(out)
    }

    fn flush(
        &mut self,
        pending: &mut String,
        line: usize,
        file: Option<&str>,
        out: &mut String,
    ) -> Result<(), PreprocessError> {
        if pending.is_empty() {
            return Ok(());
        }
        let mut ctx = Ctx { file, line };
        let toks = self
            .expand(tokenize(pending), &mut ctx, &mut Vec::new(), true)
            .map_err(|kind| PreprocessError {
                file: file.map(str::to_owned),
                line: ctx.line,
                kind,
            })?;
        for tok in &toks {
            out.push_str(&tok.text);
        }
        pending.clear();
        Ok(())
    }

    fn directive(
        &mut self,
        text: &str,
        line: usize,
        file: Option<&str>,
        conds: &mut Vec<Cond>,
        out: &mut String,
    ) -> Result<(), PreprocessError> {
        let err = |kind| PreprocessError {
            file: file.map(str::to_owned),
            line,
            kind,
        };
        let text = text.trim_start();
        let name_len = text
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(text.len());
        let (name, rest) = text.split_at(name_len);
        let active = conds.last().is_none_or(|c| c.active);

        match name {
            "if" | "ifdef" | "ifndef" => {
                let value = if !active {
                    false
                } else if name == "if" {
                    self.eval_condition(rest, line, file)?
                } else {
                    let macro_name = first_ident(rest).ok_or_else(|| {
                        err(PreprocessErrorKind::MissingMacroName(name.to_owned()))
                    })?;
                    self.is_defined(macro_name) == (name == "ifdef")
                };
                conds.push(Cond {
                    parent_active: active,
                    taken: value,
                    active: value,
                    seen_else: false,
                    line,
                });
            }
            "elif" => {
                let cond = conds
                    .last_mut()
                    .ok_or_else(|| err(PreprocessErrorKind::Unmatched(name.to_owned())))?;
                if cond.seen_else {
                    return Err(err(PreprocessErrorKind::AfterElse(name.to_owned())));
                }
                if cond.parent_active && !cond.taken {
                    let value = self.eval_condition(rest, line, file)?;
                    cond.active = value;
                    cond.taken = value;
                } else {
                    cond.active = false;
                }
            }
            "else" => {
                let cond = conds
                    .last_mut()
                    .ok_or_else(|| err(PreprocessErrorKind::Unmatched(name.to_owned())))?;
                if cond.seen_else {
                    return Err(err(PreprocessErrorKind::AfterElse(name.to_owned())));
                }
                cond.active = cond.parent_active && !cond.taken;
                cond.taken = true;
                cond.seen_else = true;
            }
            "endif" => {
                conds
                    .pop()
                    .ok_or_else(|| err(PreprocessErrorKind::Unmatched(name.to_owned())))?;
            }
            _ if !active => {}
            "define" => self.define_directive(rest).map_err(err)?,
            "undef" => {
                let macro_name = first_ident(rest)
                    .ok_or_else(|| err(PreprocessErrorKind::MissingMacroName(name.to_owned())))?;
                self.macros.remove(macro_name);
            }
            "include" => {
                let path = include_path(rest).ok_or_else(|| {
                    err(PreprocessErrorKind::InvalidInclude(rest.trim().to_owned()))
                })?;
                if self.depth >= MAX_INCLUDE_DEPTH {
                    return Err(err(PreprocessErrorKind::IncludeDepth(MAX_INCLUDE_DEPTH)));
                }
                let included = self
                    .resolve(path, file)
                    .map_err(err)?
                    .ok_or_else(|| err(PreprocessErrorKind::IncludeNotFound(path.to_owned())))?;
                self.depth += 1;
                let result = self.run(&included.content, Some(&included.path));
                self.depth -= 1;
                out.push_str(&result?);
            }
            // `#line` is written by the game's preprocessor output, `#pragma` by HEMTT
            "line" | "pragma" | "" => {}
            _ => return Err(err(PreprocessErrorKind::UnknownDirective(name.to_owned()))),
        }
        Ok(())
    }

    fn resolve(
        &self,
        path: &str,
        file: Option<&str>,
    ) -> Result<Option<IncludeFile>, PreprocessErrorKind> {
        let resolver = self
            .resolver
            .as_ref()
            .ok_or_else(|| PreprocessErrorKind::NoIncludeResolver(path.to_owned()))?;
        resolver
            .resolve(path, file)
            .map_err(|source| PreprocessErrorKind::IncludeIo {
                path: path.to_owned(),
                source,
            })
    }

    /// Parses `NAME body`, `NAME(a, b) body` or `NAME(a, ...) body`.
    fn define_directive(&mut self, text: &str) -> Result<(), PreprocessErrorKind> {
        let text = text.trim_start_matches([' ', '\t']);
        let name_len = text
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(text.len());
        let (name, mut rest) = text.split_at(name_len);
        if name.is_empty() || name.as_bytes()[0].is_ascii_digit() {
            return Err(PreprocessErrorKind::MissingMacroName("define".to_owned()));
        }

        let mut params = None;
        let mut variadic = false;
        if let Some(after) = rest.strip_prefix('(') {
            let close = after.find(')').ok_or(PreprocessErrorKind::InvalidParams)?;
            let list = after[..close].trim();
            let mut names = Vec::new();
            if !list.is_empty() {
                let parts: Vec<&str> = list.split(',').map(str::trim).collect();
                for (i, part) in parts.iter().enumerate() {
                    if *part == "..." && i + 1 == parts.len() {
                        variadic = true;
                    } else if first_ident(part) == Some(part) {
                        names.push((*part).to_owned());
                    } else {
                        return Err(PreprocessErrorKind::InvalidParams);
                    }
                }
            }
            params = Some(names);
            rest = &after[close + 1..];
        }

        // Spaces between the name and the body are dropped, tabs are not
        let body = rest.trim_start_matches(' ').trim_end();
        self.macros.insert(
            name.to_owned(),
            Rc::new(Macro {
                params,
                variadic,
                body: tokenize(body),
            }),
        );
        Ok(())
    }

    fn expand(
        &mut self,
        input: Vec<Tok>,
        ctx: &mut Ctx<'_>,
        disabled: &mut Vec<String>,
        top: bool,
    ) -> Result<Vec<Tok>, PreprocessErrorKind> {
        let mut input: VecDeque<Tok> = input.into();
        let mut out = Vec::with_capacity(input.len());

        while let Some(tok) = input.pop_front() {
            match tok.kind {
                Kind::Newline => {
                    if top {
                        ctx.line += 1;
                    }
                    out.push(tok);
                    continue;
                }
                Kind::Ident if !disabled.contains(&tok.text) => {}
                _ => {
                    out.push(tok);
                    continue;
                }
            }

            let Some(mac) = self.macros.get(&tok.text).cloned() else {
                match self.builtin(&tok.text, ctx) {
                    Some(toks) => out.extend(toks),
                    None => out.push(tok),
                }
                continue;
            };

            let mut newlines = 0;
            let body = match &mac.params {
                None => self.substitute(&mac, &[])?,
                Some(params) => {
                    match input.iter().position(|t| t.kind != Kind::Space) {
                        Some(p) if input[p].is("(") => {
                            input.drain(..=p);
                        }
                        _ => {
                            out.push(tok);
                            continue;
                        }
                    }
                    let mut args = collect_args(&mut input, &mut newlines)
                        .ok_or_else(|| PreprocessErrorKind::UnterminatedCall(tok.text.clone()))?;
                    if params.is_empty()
                        && args.len() == 1
                        && args[0].iter().all(|t| t.kind == Kind::Space)
                    {
                        args.clear();
                    }
                    let count_ok = if mac.variadic {
                        args.len() >= params.len()
                    } else {
                        args.len() == params.len()
                    };
                    if !count_ok {
                        return Err(PreprocessErrorKind::ArgCount {
                            name: tok.text.clone(),
                            expected: params.len(),
                            variadic: mac.variadic,
                            found: args.len(),
                        });
                    }
                    let mut expanded = Vec::with_capacity(args.len());
                    for arg in args {
                        expanded.push(self.expand(arg, ctx, disabled, false)?);
                    }
                    self.substitute(&mac, &expanded)?
                }
            };

            disabled.push(tok.text.clone());
            let result = self.expand(body, ctx, disabled, false);
            disabled.pop();
            let mut result = result?;

            // A function-like macro name at the end of the expansion takes its arguments from
            // the input that follows, as in `#define F G` with `F(1)`
            if let Some(last) = result.iter().rposition(|t| t.kind != Kind::Space)
                && result[last].kind == Kind::Ident
                && !disabled.contains(&result[last].text)
                && self
                    .macros
                    .get(&result[last].text)
                    .is_some_and(|m| m.params.is_some())
                && input
                    .iter()
                    .find(|t| t.kind != Kind::Space)
                    .is_some_and(|t| t.is("("))
            {
                input.push_front(result.swap_remove(last));
                result.truncate(last);
            }

            out.extend(result);
            if top {
                ctx.line += newlines;
                out.extend(std::iter::repeat_n(Tok::new(Kind::Newline, "\n"), newlines));
            }
        }
        Ok(out)
    }

    /// Replaces the arguments in the body of `mac`, then applies `##`.
    fn substitute(&self, mac: &Macro, args: &[Vec<Tok>]) -> Result<Vec<Tok>, PreprocessErrorKind> {
        let fixed = mac.params.as_ref().map_or(0, Vec::len);
        let va = args.get(fixed..).unwrap_or_default();
        let toks = self.substitute_toks(mac, &mac.body, args, va)?;
        Ok(paste(toks))
    }

    fn substitute_toks(
        &self,
        mac: &Macro,
        body: &[Tok],
        args: &[Vec<Tok>],
        va: &[Vec<Tok>],
    ) -> Result<Vec<Tok>, PreprocessErrorKind> {
        let mut res = Vec::with_capacity(body.len());
        let mut k = 0;
        while k < body.len() {
            let tok = &body[k];
            k += 1;

            if tok.kind == Kind::Hash {
                // `#arg` stringifies, a `#` before anything else is dropped
                if let Some(p) = body.get(k).and_then(|n| mac.param(n)) {
                    res.push(Tok::new(Kind::Str, format!("\"{}\"", join(&args[p]))));
                    k += 1;
                } else if mac.variadic && body.get(k).is_some_and(|n| n.text == "__VA_ARGS__") {
                    res.push(Tok::new(Kind::Str, format!("\"{}\"", join(&join_va(va)))));
                    k += 1;
                }
                continue;
            }

            if let Some(p) = mac.param(tok) {
                res.extend(args[p].iter().cloned());
                continue;
            }

            if !mac.variadic || tok.kind != Kind::Ident || !tok.text.starts_with("__VA_") {
                res.push(tok.clone());
                continue;
            }

            if tok.text == "__VA_ARGS__" {
                res.extend(join_va(va));
                continue;
            }

            let call = next_non_space(body, k)
                .filter(|&p| body[p].is("("))
                .and_then(|p| split_args(body, p));
            let Some((call_args, end)) = call else {
                res.push(tok.clone());
                continue;
            };

            match tok.text.as_str() {
                "__VA_OPT__" | "__VA_OPT_INV__" => {
                    if va.is_empty() == (tok.text == "__VA_OPT_INV__") {
                        let inner = &body[next_non_space(body, k).unwrap_or(k) + 1..end - 1];
                        res.extend(self.substitute_toks(mac, inner, args, va)?);
                    }
                }
                "__VA_SELECT__" => {
                    let index = self.substitute_toks(mac, &call_args[0], args, va)?;
                    let index = join(&index);
                    let selected = index
                        .trim()
                        .parse::<usize>()
                        .ok()
                        .and_then(|i| va.get(i))
                        .ok_or_else(|| PreprocessErrorKind::InvalidVaSelect(index.clone()))?;
                    res.extend(selected.iter().cloned());
                }
                "__VA_APPLY__" => {
                    let func = match call_args.first().map(Vec::as_slice) {
                        Some([func]) if func.kind == Kind::Ident => func,
                        _ => {
                            return Err(PreprocessErrorKind::InvalidVaApply(join(
                                &call_args.concat(),
                            )));
                        }
                    };
                    let arity = self
                        .macros
                        .get(&func.text)
                        .ok_or_else(|| PreprocessErrorKind::InvalidVaApply(func.text.clone()))?
                        .params
                        .as_ref()
                        .map_or(0, Vec::len);
                    let sep = match call_args.get(1) {
                        Some(sep) => self.substitute_toks(mac, sep, args, va)?,
                        None => vec![Tok::new(Kind::Other, ",")],
                    };
                    for (i, arg) in va.iter().enumerate() {
                        if i > 0 {
                            res.extend(sep.iter().cloned());
                        }
                        res.push(func.clone());
                        if arity >= 1 {
                            res.push(Tok::new(Kind::Other, "("));
                            res.extend(arg.iter().cloned());
                            if arity >= 2 {
                                res.push(Tok::new(Kind::Other, ","));
                                res.push(Tok::new(Kind::Other, i.to_string()));
                            }
                            res.push(Tok::new(Kind::Other, ")"));
                        }
                    }
                }
                _ => {
                    res.push(tok.clone());
                    continue;
                }
            }
            k = end;
        }
        Ok(res)
    }

    fn builtin(&mut self, name: &str, ctx: &Ctx<'_>) -> Option<Vec<Tok>> {
        if !name.starts_with("__") {
            return None;
        }
        let file = ctx.file.unwrap_or_default();
        let file_name = file.rsplit(['\\', '/']).next().unwrap_or_default();
        let other = |text: String| Some(vec![Tok::new(Kind::Other, text)]);
        let string = |text: &str| Some(vec![Tok::new(Kind::Str, format!("\"{text}\""))]);

        match name {
            "__LINE__" => other(ctx.line.to_string()),
            "__FILE__" => string(file),
            "__FILE_NAME__" => string(file_name),
            "__FILE_SHORT__" => string(
                file_name
                    .rsplit_once('.')
                    .map_or(file_name, |(stem, _)| stem),
            ),
            "__COUNTER__" => {
                self.counter += 1;
                other((self.counter - 1).to_string())
            }
            "__COUNTER_RESET__" => {
                self.counter = 0;
                Some(Vec::new())
            }
            "__RAND_INT8__" => other(rand::random::<i8>().to_string()),
            "__RAND_INT16__" => other(rand::random::<i16>().to_string()),
            "__RAND_INT32__" => other(rand::random::<i32>().to_string()),
            "__RAND_INT64__" => other(rand::random::<i64>().to_string()),
            "__RAND_UINT8__" => other(rand::random::<u8>().to_string()),
            "__RAND_UINT16__" => other(rand::random::<u16>().to_string()),
            "__RAND_UINT32__" => other(rand::random::<u32>().to_string()),
            "__RAND_UINT64__" => other(rand::random::<u64>().to_string()),
            _ => {
                let ts = self.timestamp.unwrap_or_else(|| {
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs())
                });
                let (year, month, day) = civil_from_days((ts / 86_400) as i64);
                let secs = ts % 86_400;
                let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
                match name {
                    "__DATE_ARR__" => other(format!("{year},{month},{day},{h},{m},{s}")),
                    "__DATE_STR__" => {
                        string(&format!("{year}/{month:02}/{day:02}, {h:02}:{m:02}:{s:02}"))
                    }
                    "__DATE_STR_ISO8601__" => {
                        string(&format!("{year}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z"))
                    }
                    "__TIME__" | "__TIME_UTC__" => other(format!("{h:02}:{m:02}:{s:02}")),
                    "__DAY__" => other(day.to_string()),
                    "__MONTH__" => other(month.to_string()),
                    "__YEAR__" => other(year.to_string()),
                    "__TIMESTAMP_UTC__" => other(ts.to_string()),
                    _ => None,
                }
            }
        }
    }

    fn eval_condition(
        &mut self,
        expr: &str,
        line: usize,
        file: Option<&str>,
    ) -> Result<bool, PreprocessError> {
        let err = |kind| PreprocessError {
            file: file.map(str::to_owned),
            line,
            kind,
        };
        let toks = tokenize(expr);

        // `defined(X)` and `__has_include("...")` work on the raw tokens
        let mut pre = Vec::with_capacity(toks.len());
        let mut i = 0;
        while i < toks.len() {
            let tok = &toks[i];
            if tok.kind == Kind::Ident && (tok.text == "defined" || tok.text == "__has_include") {
                let (arg, end) = match next_non_space(&toks, i + 1) {
                    Some(p) if toks[p].is("(") => {
                        let (args, end) = split_args(&toks, p).ok_or_else(|| {
                            err(PreprocessErrorKind::InvalidCondition(
                                expr.trim().to_owned(),
                            ))
                        })?;
                        (join(&args.concat()), end)
                    }
                    Some(p) if tok.text == "defined" => (toks[p].text.clone(), p + 1),
                    _ => {
                        return Err(err(PreprocessErrorKind::InvalidCondition(
                            expr.trim().to_owned(),
                        )));
                    }
                };
                let value = if tok.text == "defined" {
                    self.is_defined(arg.trim())
                } else {
                    // The game only finds absolute paths
                    match include_path(&arg) {
                        Some(path) if path.starts_with('\\') => {
                            self.resolver.is_some()
                                && self.resolve(path, file).map_err(err)?.is_some()
                        }
                        Some(_) => false,
                        None => {
                            return Err(err(PreprocessErrorKind::InvalidInclude(
                                arg.trim().to_owned(),
                            )));
                        }
                    }
                };
                pre.push(Tok::new(Kind::Other, if value { "1" } else { "0" }));
                i = end;
                continue;
            }
            pre.push(tok.clone());
            i += 1;
        }

        let mut ctx = Ctx { file, line };
        let expanded = self
            .expand(pre, &mut ctx, &mut Vec::new(), false)
            .map_err(err)?;
        let text = join(&expanded);
        eval_expr(&text).map(|v| v != 0.0).ok_or_else(|| {
            err(PreprocessErrorKind::InvalidCondition(
                expr.trim().to_owned(),
            ))
        })
    }
}

/// Collects the arguments of a macro call, the `(` is already consumed.
fn collect_args(input: &mut VecDeque<Tok>, newlines: &mut usize) -> Option<Vec<Vec<Tok>>> {
    let mut args = Vec::new();
    let mut cur = Vec::new();
    let mut depth = 0usize;
    while let Some(tok) = input.pop_front() {
        if tok.kind == Kind::Newline {
            *newlines += 1;
            cur.push(Tok::new(Kind::Space, " "));
            continue;
        }
        if tok.is("(") {
            depth += 1;
        } else if tok.is(")") {
            if depth == 0 {
                args.push(cur);
                return Some(args);
            }
            depth -= 1;
        } else if tok.is(",") && depth == 0 {
            args.push(std::mem::take(&mut cur));
            continue;
        }
        cur.push(tok);
    }
    None
}

fn join_va(va: &[Vec<Tok>]) -> Vec<Tok> {
    let mut res = Vec::new();
    for (i, arg) in va.iter().enumerate() {
        if i > 0 {
            res.push(Tok::new(Kind::Other, ","));
        }
        res.extend(arg.iter().cloned());
    }
    res
}

/// Applies `##`. Unlike in C it only separates words and is removed, whitespace around it
/// stays: `class ##name` gives `class value`, `a##b` gives `ab`. The result is tokenized
/// again so glued words are found when the expansion is rescanned.
fn paste(toks: Vec<Tok>) -> Vec<Tok> {
    if !toks.iter().any(|t| t.kind == Kind::HashHash) {
        return toks;
    }
    let text: String = toks
        .iter()
        .filter(|t| t.kind != Kind::HashHash)
        .map(|t| t.text.as_str())
        .collect();
    tokenize(&text)
}

fn first_ident(text: &str) -> Option<&str> {
    let text = text.trim_start();
    let len = text
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(text.len());
    let ident = &text[..len];
    (!ident.is_empty() && !ident.as_bytes()[0].is_ascii_digit()).then_some(ident)
}

/// The path of `"path"` or `<path>`. Macros are not expanded in include paths.
fn include_path(text: &str) -> Option<&str> {
    let text = text.trim();
    let close = match text.chars().next()? {
        '"' => '"',
        '<' => '>',
        _ => return None,
    };
    let inner = &text[1..];
    let end = inner.find(close)?;
    let path = &inner[..end];
    (!path.is_empty()).then_some(path)
}

/// Days since 1970-01-01 to (year, month, day).
const fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

/// Evaluates an `#if` expression: numbers, `!`, unary `-`, `* / %`, `+ -`, comparisons,
/// `&&`, `||` and parentheses. Unknown identifiers are 0.
fn eval_expr(text: &str) -> Option<f64> {
    let mut eval = Eval {
        s: text.as_bytes(),
        i: 0,
    };
    let value = eval.or()?;
    eval.skip_ws();
    (eval.i == eval.s.len()).then_some(value)
}

struct Eval<'a> {
    s: &'a [u8],
    i: usize,
}

impl Eval<'_> {
    const fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn eat(&mut self, op: &str) -> bool {
        self.skip_ws();
        if self.s[self.i..].starts_with(op.as_bytes()) {
            // `<` must not match `<=`, `!` not `!=`, `=` alone is not an operator
            let next = self.s.get(self.i + op.len());
            if matches!(op, "<" | ">" | "!") && next == Some(&b'=') {
                return false;
            }
            self.i += op.len();
            true
        } else {
            false
        }
    }

    fn or(&mut self) -> Option<f64> {
        let mut v = self.and()?;
        while self.eat("||") {
            let r = self.and()?;
            v = f64::from(u8::from(v != 0.0 || r != 0.0));
        }
        Some(v)
    }

    fn and(&mut self) -> Option<f64> {
        let mut v = self.eq()?;
        while self.eat("&&") {
            let r = self.eq()?;
            v = f64::from(u8::from(v != 0.0 && r != 0.0));
        }
        Some(v)
    }

    fn eq(&mut self) -> Option<f64> {
        let mut v = self.rel()?;
        loop {
            if self.eat("==") {
                v = f64::from(u8::from(v == self.rel()?));
            } else if self.eat("!=") {
                v = f64::from(u8::from(v != self.rel()?));
            } else {
                return Some(v);
            }
        }
    }

    fn rel(&mut self) -> Option<f64> {
        let mut v = self.add()?;
        loop {
            if self.eat("<=") {
                v = f64::from(u8::from(v <= self.add()?));
            } else if self.eat(">=") {
                v = f64::from(u8::from(v >= self.add()?));
            } else if self.eat("<") {
                v = f64::from(u8::from(v < self.add()?));
            } else if self.eat(">") {
                v = f64::from(u8::from(v > self.add()?));
            } else {
                return Some(v);
            }
        }
    }

    fn add(&mut self) -> Option<f64> {
        let mut v = self.mul()?;
        loop {
            if self.eat("+") {
                v += self.mul()?;
            } else if self.eat("-") {
                v -= self.mul()?;
            } else {
                return Some(v);
            }
        }
    }

    fn mul(&mut self) -> Option<f64> {
        let mut v = self.unary()?;
        loop {
            if self.eat("*") {
                v *= self.unary()?;
            } else if self.eat("/") {
                v /= self.unary()?;
            } else if self.eat("%") {
                v %= self.unary()?;
            } else {
                return Some(v);
            }
        }
    }

    fn unary(&mut self) -> Option<f64> {
        if self.eat("!") {
            return Some(f64::from(u8::from(self.unary()? == 0.0)));
        }
        if self.eat("-") {
            return Some(-self.unary()?);
        }
        if self.eat("+") {
            return self.unary();
        }
        self.primary()
    }

    fn primary(&mut self) -> Option<f64> {
        if self.eat("(") {
            let v = self.or()?;
            return self.eat(")").then_some(v);
        }
        self.skip_ws();
        let start = self.i;
        while self.i < self.s.len()
            && (self.s[self.i].is_ascii_alphanumeric() || matches!(self.s[self.i], b'_' | b'.'))
        {
            self.i += 1;
        }
        let word = std::str::from_utf8(&self.s[start..self.i]).ok()?;
        let first = *word.as_bytes().first()?;
        if first.is_ascii_alphabetic() || first == b'_' {
            return Some(0.0);
        }
        if let Some(hex) = word.strip_prefix("0x").or_else(|| word.strip_prefix("0X")) {
            return i64::from_str_radix(hex, 16).ok().map(|v| v as f64);
        }
        word.parse().ok()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn pp(src: &str) -> String {
        Preprocessor::new().preprocess(src, None).unwrap()
    }

    fn pp_err(src: &str) -> PreprocessErrorKind {
        Preprocessor::new().preprocess(src, None).unwrap_err().kind
    }

    #[test]
    fn object_like() {
        assert_eq!(pp("#define true 1\na = true;"), "\na = 1;");
        assert_eq!(pp("#define MACRO test\nMACRO"), "\ntest");
        assert_eq!(pp("#define MACRO\ttest\nMACRO"), "\n\ttest");
        assert_eq!(pp("#define MACRO#test\nMACRO"), "\ntest");
        assert_eq!(pp("#define MACRO Test #123\nMACRO"), "\nTest 123");
        assert_eq!(pp("#define EMPTY\na EMPTY b"), "\na  b");
    }

    #[test]
    fn strings() {
        assert_eq!(
            pp("#define ARG world\na = 'hello ARG';\nb = \"hello ARG\";"),
            "\na = 'hello world';\nb = \"hello ARG\";"
        );
        assert_eq!(
            pp("#define A x\nb = \"\"\"A\"\"\" A;"),
            "\nb = \"\"\"A\"\"\" x;"
        );
    }

    #[test]
    fn comments() {
        assert_eq!(pp("a = 1; // c\nb = 2;"), "a = 1; \nb = 2;");
        assert_eq!(
            pp("a[] = { \"apple\"/*, \"banana\"*/, \"pear\" };"),
            "a[] = { \"apple\" , \"pear\" };"
        );
        assert_eq!(pp("/*\n x\n*/b"), "\n\nb");
        assert_eq!(pp("a = \"//not a comment\";"), "a = \"//not a comment\";");
    }

    #[test]
    fn arguments() {
        assert_eq!(
            pp("#define CAR(NAME) displayName = NAME;\nCAR(\"Mini\")"),
            "\ndisplayName = \"Mini\";"
        );
        assert_eq!(
            pp("#define BLASTOFF(UNIT,RATE) UNIT setVelocity [0,0,RATE];\nBLASTOFF(player, 10)"),
            "\nplayer setVelocity [0,0, 10];"
        );
        assert_eq!(
            pp("#define ONE foo\n#define TWO(ONE) ONE\nTWO(bar)"),
            "\n\nbar"
        );
        assert_eq!(pp("#define M(a) <a>\nM((1,2))"), "\n<(1,2)>");
        assert_eq!(
            pp("#define M(a) <a>\nM(\"Some, content\")"),
            "\n<\"Some, content\">"
        );
        // A function-like macro without arguments is left alone
        assert_eq!(pp("#define M(a) a\nclass M {};"), "\nclass M {};");
        assert_eq!(pp("#define M() x\nM()"), "\nx");
    }

    #[test]
    fn stringify_and_concat() {
        let src = "#define STRINGIFY(s) #s\n#define FOO 123\ntest1 = STRINGIFY(123);\ntest2 = STRINGIFY(FOO);";
        assert_eq!(pp(src), "\n\ntest1 = \"123\";\ntest2 = \"123\";");
        let src = "#define GLUE(g1,g2) g1##g2\n#define FOO 123\n#define BAR 456\na = GLUE(123,456);\nb = GLUE(FOO,BAR);";
        assert_eq!(pp(src), "\n\n\na = 123456;\nb = 123456;");
        let src = "#define M(NAME,FOLDER) class NAME##_Button: RscText { model = \\OFP2\\##FOLDER##\\##FOLDER; };\nM(Big,dir)";
        assert_eq!(
            pp(src),
            "\nclass Big_Button: RscText { model = \\OFP2\\dir\\dir; };"
        );
        // `##` keeps the whitespace around it, ACE writes `class ##weapon`
        let src = "#define GVAR(x) ace_csw_##x\n\
                   #define PROXY(weapon) class ##weapon; class GVAR(weapon): ##weapon {}\nPROXY(m29);";
        assert_eq!(pp(src), "\n\nclass m29; class ace_csw_m29: m29 {};");
    }

    #[test]
    fn cba_style() {
        let src = "#define PREFIX cba\n#define COMPONENT main\n\
                   #define DOUBLES(var1,var2) var1##_##var2\n\
                   #define ADDON DOUBLES(PREFIX,COMPONENT)\n\
                   #define GVAR(var1) DOUBLES(ADDON,var1)\n\
                   #define QUOTE(var1) #var1\n\
                   #define ARR_2(a,b) a,b\n\
                   a = QUOTE(GVAR(x));\nb[] = {ARR_2(1,2)};";
        assert_eq!(pp(src), "\n\n\n\n\n\n\na = \"cba_main_x\";\nb[] = {1,2};");
    }

    #[test]
    fn multi_line() {
        let src = "#define M(a) class a \\\n{ \\\n  x = 1; \\\n};\nM(B)\nend";
        let out = pp(src);
        assert!(out.starts_with("\n\n\n\nclass B"), "{out:?}");
        assert!(out.ends_with("};\nend"));
        // Calls spanning lines keep the line count
        assert_eq!(pp("#define M(a,b) a+b\nM(1,\n2)\n__LINE__"), "\n1+ 2\n\n4");
    }

    #[test]
    fn rescan() {
        assert_eq!(pp("#define F G\n#define G(x) <x>\nF(1)"), "\n\n<1>");
        assert_eq!(pp("#define A A B\nA"), "\nA B");
        assert_eq!(pp("#define A B\n#define B A\nA"), "\n\nA");
    }

    #[test]
    fn variadic() {
        assert_eq!(
            pp("#define M(...) __VA_ARGS__\nM()|M(a,b,c)|M({str [1,2,3]})"),
            "\n|a,b,c|{str [1,2,3]}"
        );
        let log = "#define LOG(message, ...) diag_log __VA_OPT__([) message __VA_OPT__(,) __VA_ARGS__ __VA_OPT__(])\n";
        assert_eq!(
            pp(&format!("{log}LOG(\"My Message\")")),
            "\ndiag_log  \"My Message\"   "
        );
        assert_eq!(
            pp(&format!("{log}LOG(\"My Message\",1,2,3)")),
            "\ndiag_log [ \"My Message\" , 1,2,3 ]"
        );
        assert_eq!(
            pp("#define H(...) __VA_OPT__(yes)__VA_OPT_INV__(no)\nH() H(a)"),
            "\nno yes"
        );
        assert_eq!(
            pp("#define FUNC A\n#define T(...) __VA_APPLY__(FUNC)\nT(1,2,3)"),
            "\n\nA,A,A"
        );
        assert_eq!(
            pp("#define QUOTE(x) #x\n#define T(...) __VA_APPLY__(QUOTE)\nT(1,2,3)"),
            "\n\n\"1\",\"2\",\"3\""
        );
        assert_eq!(
            pp(
                "#define FUNC(x, __VA_INDEX__) __VA_INDEX__: #x\n#define T(...) __VA_APPLY__(FUNC)\nT(1,2,3)"
            ),
            "\n\n0: \"1\",1: \"2\",2: \"3\""
        );
        let src = "#define GET_VARIABLE(x) private _##x = player getVariable #x\n\
                   #define SEPARATOR_SEMI ;\n\
                   #define GET_VARIABLES(...) __VA_APPLY__(GET_VARIABLE, SEPARATOR_SEMI)\n\
                   GET_VARIABLES()|GET_VARIABLES(MyFnc,MyOtherFnc)";
        assert_eq!(
            pp(src),
            "\n\n\n|private _MyFnc = player getVariable \"MyFnc\";private _MyOtherFnc = player getVariable \"MyOtherFnc\""
        );
        let src = "#define QUOTE(x) #x\n#define WRITE_LOG(...) diag_log [__VA_ARGS__]\n\
                   #define MAKE_ARRAY(...) [__VA_ARGS__]\n\
                   #define DO_THINGS(...) WRITE_LOG(__VA_APPLY__(QUOTE)); _array = MAKE_ARRAY(__VA_ARGS__);\n\
                   DO_THINGS(a,b,c)";
        assert_eq!(
            pp(src),
            "\n\n\n\ndiag_log [\"a\",\"b\",\"c\"]; _array = [a,b,c];"
        );
        let src = "#define SELECT_ARG(index, ...) __VA_SELECT__(index)\n\
                   #define FORMAT_ARG(index) SELECT_ARG(index, \"A=%2\", \"B=%3\", \"C=%4\")\n\
                   #define FORMAT_GET_ARGSTR(arg, __VA_INDEX__) FORMAT_ARG(__VA_INDEX__)\n\
                   #define FORMAT(MESSAGE, ...) format [[__VA_OPT_INV__('%1') __VA_OPT__('%1:',) __VA_APPLY__(FORMAT_GET_ARGSTR)] joinString ' ', MESSAGE __VA_OPT__(,) __VA_ARGS__]\n\
                   FORMAT(\"Message\")\nFORMAT(\"Message\", value1, value2)";
        assert_eq!(
            pp(src),
            "\n\n\n\nformat [['%1'  ] joinString ' ', \"Message\"  ]\n\
             format [[ '%1:',  \"A=%2\", \"B=%3\"] joinString ' ', \"Message\" ,  value1, value2]"
        );
    }

    #[test]
    fn conditionals() {
        assert_eq!(
            pp("#define A\n#ifdef A\nyes\n#else\nno\n#endif"),
            "\n\nyes\n\n\n"
        );
        assert_eq!(pp("#ifndef A\nyes\n#else\nno\n#endif"), "\nyes\n\n\n");
        assert_eq!(
            pp("#ifdef A\n#ifdef B\nx\n#else\ny\n#endif\n#else\nz\n#endif"),
            "\n\n\n\n\n\n\nz\n"
        );
        assert_eq!(
            pp("#ifdef A\n#define X\n#include \"missing\"\n#bogus\n#endif\nX"),
            "\n\n\n\n\nX"
        );
        let mut p = Preprocessor::new();
        p.define("__GAME_VER_MAJ__", "02").unwrap();
        p.define("__GAME_BUILD__", "151036").unwrap();
        let src = "#if __GAME_VER_MAJ__ != 2\na\n#elif __GAME_BUILD__ > 151035 && __ARMA3__\nb\n#else\nc\n#endif";
        assert_eq!(p.preprocess(src, None).unwrap(), "\n\n\nb\n\n\n");
        assert_eq!(pp("#if __A3_PROFILING__\na\n#endif"), "\n\n");
        assert_eq!(
            pp("#if defined(__ARMA__) && !defined X\na\n#endif"),
            "\na\n"
        );
        assert_eq!(
            pp(
                "#if __has_include(\"\\z\\ace\\addons\\main\\script_component.hpp\")\na\n#else\nb\n#endif"
            ),
            "\n\n\nb\n"
        );
    }

    #[test]
    fn errors() {
        assert!(matches!(pp_err("#endif"), PreprocessErrorKind::Unmatched(d) if d == "endif"));
        assert!(matches!(
            pp_err("#ifdef A\n"),
            PreprocessErrorKind::UnterminatedIf
        ));
        assert!(
            matches!(pp_err("#inlcude \"x\""), PreprocessErrorKind::UnknownDirective(d) if d == "inlcude")
        );
        assert!(matches!(
            pp_err("#include x"),
            PreprocessErrorKind::InvalidInclude(_)
        ));
        assert!(matches!(
            pp_err("#include \"x.hpp\""),
            PreprocessErrorKind::NoIncludeResolver(_)
        ));
        assert!(matches!(
            pp_err("#define M(a) a\nM(1,2)"),
            PreprocessErrorKind::ArgCount { found: 2, .. }
        ));
        assert!(matches!(
            pp_err("#define M(a) a\nM(1"),
            PreprocessErrorKind::UnterminatedCall(_)
        ));
        assert!(matches!(
            pp_err("#if 1 +\n#endif"),
            PreprocessErrorKind::InvalidCondition(_)
        ));
        let err = Preprocessor::new()
            .preprocess("a\nb\n#else", Some("x.hpp"))
            .unwrap_err();
        assert_eq!(err.to_string(), "x.hpp:3: `#else` without a matching #if");
    }

    #[test]
    fn builtins() {
        let mut p = Preprocessor::new();
        p.set_timestamp(Some(1_603_898_262));
        let src = "__LINE__ __FILE__ __FILE_NAME__ __FILE_SHORT__\n__COUNTER__ __COUNTER__ __COUNTER_RESET____COUNTER__\n\
                   __DATE_ARR__|__DATE_STR__|__DATE_STR_ISO8601__|__TIME__|__DAY__ __MONTH__ __YEAR__ __TIMESTAMP_UTC__";
        assert_eq!(
            p.preprocess(src, Some("userconfig\\file1.sqf")).unwrap(),
            "1 \"userconfig\\file1.sqf\" \"file1.sqf\" \"file1\"\n0 1 __COUNTER_RESET____COUNTER__\n\
             2020,10,28,15,17,42|\"2020/10/28, 15:17:42\"|\"2020-10-28T15:17:42Z\"|15:17:42|28 10 2020 1603898262"
        );
        assert_eq!(pp("__COUNTER_RESET__ __COUNTER__"), " 0");
        assert!(pp("__RAND_UINT8__").parse::<u8>().is_ok());
        assert!(pp("__RAND_INT64__").parse::<i64>().is_ok());
    }

    #[test]
    fn includes() {
        let files: HashMap<&str, &str> = [
            ("\\x\\mod\\addons\\main\\config.cpp", "#include \"script_component.hpp\"\nvalue = VALUE;\n#include \"..\\common\\other.hpp\""),
            ("\\x\\mod\\addons\\main\\script_component.hpp", "#include \"\\x\\mod\\addons\\main\\macros.hpp\"\n#define VALUE QUOTE(abc)"),
            ("\\x\\mod\\addons\\main\\macros.hpp", "#define QUOTE(x) #x"),
            ("\\x\\mod\\addons\\common\\other.hpp", "other = __LINE__; name = __FILE_NAME__;\n#if __has_include(\"\\x\\mod\\addons\\main\\macros.hpp\")\nfound = 1;\n#endif"),
            ("\\loop.hpp", "#include \"\\loop.hpp\""),
        ]
        .into_iter()
        .collect();
        let resolver = |path: &str, from: Option<&str>| {
            let path = join_include_path(path, from);
            Ok(files.get(path.as_str()).map(|content| IncludeFile {
                path,
                content: (*content).to_owned(),
            }))
        };
        let mut p = Preprocessor::new().with_resolver(resolver);
        let out = p
            .preprocess_path("\\x\\mod\\addons\\main\\config.cpp")
            .unwrap();
        assert_eq!(
            out,
            "\n\nvalue = \"abc\";\nother = 1; name = \"other.hpp\";\n\nfound = 1;\n"
        );

        let err = p
            .preprocess("#include \"nope.hpp\"", Some("\\a\\b.cpp"))
            .unwrap_err();
        assert!(matches!(err.kind, PreprocessErrorKind::IncludeNotFound(ref p) if p == "nope.hpp"));
        let err = p.preprocess_path("\\loop.hpp").unwrap_err();
        assert!(matches!(
            err.kind,
            PreprocessErrorKind::IncludeDepth(MAX_INCLUDE_DEPTH)
        ));
    }

    #[test]
    fn include_paths() {
        assert_eq!(
            join_include_path("a.hpp", Some("\\x\\m\\config.cpp")),
            "\\x\\m\\a.hpp"
        );
        assert_eq!(
            join_include_path("..\\a.hpp", Some("\\x\\m\\config.cpp")),
            "\\x\\a.hpp"
        );
        assert_eq!(
            join_include_path("\\a3\\b.hpp", Some("\\x\\m\\config.cpp")),
            "\\a3\\b.hpp"
        );
        assert_eq!(
            join_include_path("sub/a.hpp", Some("x\\config.cpp")),
            "x\\sub\\a.hpp"
        );
        assert_eq!(join_include_path("a.hpp", None), "a.hpp");
        assert_eq!(join_include_path("D:\\file.txt", None), "D:\\file.txt");
        assert_eq!(include_path("<file.txt>"), Some("file.txt"));
        assert_eq!(include_path("\"\\a\\b.hpp\" // x"), Some("\\a\\b.hpp"));
    }

    #[test]
    fn line_numbers_survive() {
        let src = "#define A 1\n/* x\ny */\na = A;\nb = __LINE__;";
        assert_eq!(pp(src), "\n\n\na = 1;\nb = 5;");
    }
}
