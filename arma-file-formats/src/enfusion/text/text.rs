use crate::errors::{AffError, TextError};

/// An object: a class, optionally inheriting from another resource, with its entries.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Object {
    /// Class name, `None` for plain blocks like `Refs { ... }`.
    pub class: Option<String>,
    /// Resource this object inherits from (`Class : "{GUID}path" { ... }`), as written.
    pub parent: Option<String>,
    /// Object id, e.g. the `"{51823C7810E8F337}"` of `Key MatUVTransform "{51823C7810E8F337}" {`.
    pub id: Option<String>,
    pub entries: Vec<Entry>,
}

/// One line of an object: a key with values, or a key with a nested object.
///
/// Lines without a key, like the items of a list (`"Enabled"`) or matrix rows (`1 0 0 0`),
/// use their first token as key, so nothing is lost.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub key: String,
    pub values: Vec<Value>,
    pub object: Option<Object>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A quoted string, without the quotes.
    String(String),
    /// Anything unquoted: numbers, identifiers, flags.
    Word(String),
    /// `name( args )`, e.g. `anim( 20 )`.
    Call { name: String, args: Vec<Self> },
}

impl Value {
    /// The text of a string or word.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) | Self::Word(text) => Some(text),
            Self::Call { .. } => None,
        }
    }

    #[must_use]
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Self::Word(word) => word.parse().ok(),
            _ => None,
        }
    }
}

impl Object {
    /// Parses a document consisting of a single root object.
    pub fn parse(text: &str) -> Result<Self, AffError> {
        let mut parser = Parser::new(text);
        parser.skip_newlines();
        let object = parser.root()?;
        parser.skip_newlines();
        parser.next().map_or(Ok(object), |token| {
            Err(parser.error(format!("unexpected {token:?} after the root object")))
        })
    }

    /// The first entry with this key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.key == key)
    }

    /// The values of the first entry with this key.
    #[must_use]
    pub fn values(&self, key: &str) -> Option<&[Value]> {
        self.get(key).map(|entry| entry.values.as_slice())
    }

    /// The first value of the entry with this key as number.
    #[must_use]
    pub fn float(&self, key: &str) -> Option<f32> {
        self.values(key)?.first()?.as_f32()
    }

    /// The nested object of the entry with this key.
    #[must_use]
    pub fn object(&self, key: &str) -> Option<&Self> {
        self.get(key)?.object.as_ref()
    }

    /// Applies `child` on top of `self`, like Enfusion inheritance: entries of the child replace
    /// entries with the same key. Nested objects with a class (e.g. `MatUVTransform`) are merged
    /// the same way, plain blocks and values are replaced as a whole.
    pub fn merge(&mut self, child: Self) {
        for entry in child.entries {
            let existing = self.entries.iter_mut().find(|e| e.key == entry.key);
            match (existing, entry.object) {
                (Some(existing), Some(object))
                    if object.class.is_some() && existing.object.is_some() =>
                {
                    existing.values = entry.values;
                    if let Some(base) = existing.object.as_mut() {
                        base.class.clone_from(&object.class);
                        base.id.clone_from(&object.id);
                        base.merge(object);
                    }
                }
                (Some(existing), object) => {
                    existing.values = entry.values;
                    existing.object = object;
                }
                (None, object) => self.entries.push(Entry {
                    key: entry.key,
                    values: entry.values,
                    object,
                }),
            }
        }
        self.class = child.class.or_else(|| self.class.take());
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    String(String),
    Word(String),
    OpenBrace,
    CloseBrace,
    OpenParen,
    CloseParen,
    Colon,
    Newline,
}

struct Parser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    peeked: Option<Token>,
    line: usize,
    /// Line the last lexed token starts on, for errors.
    token_line: usize,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            chars: text.trim_start_matches('\u{feff}').chars().peekable(),
            peeked: None,
            line: 1,
            token_line: 1,
        }
    }

    fn error(&self, message: String) -> AffError {
        TextError::Parse {
            line: self.token_line,
            message,
        }
        .into()
    }

    fn peek(&mut self) -> Option<&Token> {
        if self.peeked.is_none() {
            self.peeked = self.lex();
        }
        self.peeked.as_ref()
    }

    fn next(&mut self) -> Option<Token> {
        self.peek();
        self.peeked.take()
    }

    fn lex(&mut self) -> Option<Token> {
        while let Some(&c) = self.chars.peek() {
            self.token_line = self.line;
            if c == '\n' {
                self.chars.next();
                self.line += 1;
                return Some(Token::Newline);
            }
            if c.is_whitespace() {
                self.chars.next();
                continue;
            }
            self.chars.next();
            return Some(match c {
                '{' => Token::OpenBrace,
                '}' => Token::CloseBrace,
                '(' => Token::OpenParen,
                ')' => Token::CloseParen,
                ':' => Token::Colon,
                '"' => {
                    let mut text = String::new();
                    for c in self.chars.by_ref() {
                        if c == '"' {
                            break;
                        }
                        if c == '\n' {
                            self.line += 1;
                        }
                        text.push(c);
                    }
                    Token::String(text)
                }
                c => {
                    let mut word = c.to_string();
                    while let Some(&c) = self.chars.peek() {
                        if c.is_whitespace() || "{}():\"".contains(c) {
                            break;
                        }
                        word.push(c);
                        self.chars.next();
                    }
                    Token::Word(word)
                }
            });
        }
        None
    }

    fn skip_newlines(&mut self) {
        while self.peek() == Some(&Token::Newline) {
            self.next();
        }
    }

    /// `Class ["id"] [: "parent"] ["id"] {` followed by the body.
    fn root(&mut self) -> Result<Object, AffError> {
        let class = match self.next() {
            Some(Token::Word(class)) => class,
            other => return Err(self.error(format!("expected a class name, found {other:?}"))),
        };
        let mut object = Object {
            class: Some(class),
            ..Object::default()
        };
        loop {
            match self.next() {
                Some(Token::String(id)) if object.id.is_none() => object.id = Some(id),
                Some(Token::Colon) if object.parent.is_none() => match self.next() {
                    Some(Token::String(parent)) => object.parent = Some(parent),
                    other => {
                        return Err(self.error(format!("expected a parent, found {other:?}")));
                    }
                },
                Some(Token::OpenBrace) => break,
                other => return Err(self.error(format!("expected `{{`, found {other:?}"))),
            }
        }
        object.entries = self.body()?;
        Ok(object)
    }

    /// Entries up to and including the closing brace. Some game files lack closing braces at
    /// the end, the end of the file closes every open object.
    fn body(&mut self) -> Result<Vec<Entry>, AffError> {
        let mut entries = Vec::new();
        loop {
            match self.peek() {
                Some(Token::Newline) => {
                    self.next();
                }
                Some(Token::CloseBrace) => {
                    self.next();
                    return Ok(entries);
                }
                None => return Ok(entries),
                Some(_) => entries.push(self.entry()?),
            }
        }
    }

    /// `key values... [ { body } ]`, ending at a newline or before a closing brace.
    fn entry(&mut self) -> Result<Entry, AffError> {
        let key = match self.next() {
            Some(Token::String(key) | Token::Word(key)) => key,
            other => return Err(self.error(format!("expected a key, found {other:?}"))),
        };
        let mut values = Vec::new();
        let mut parent = None;
        loop {
            match self.peek() {
                None | Some(Token::CloseBrace) => break,
                Some(Token::Colon) => {
                    self.next();
                    match self.next() {
                        Some(Token::String(text)) => parent = Some(text),
                        other => {
                            return Err(self.error(format!("expected a parent, found {other:?}")));
                        }
                    }
                }
                Some(Token::Newline) => {
                    self.next();
                    break;
                }
                Some(Token::OpenBrace) => {
                    self.next();
                    let mut object = object_from_header(values, self.body()?);
                    object.parent = parent;
                    return Ok(Entry {
                        key,
                        values: Vec::new(),
                        object: Some(object),
                    });
                }
                Some(_) => values.push(self.value()?),
            }
        }
        Ok(Entry {
            key,
            values,
            object: None,
        })
    }

    fn value(&mut self) -> Result<Value, AffError> {
        match self.next() {
            Some(Token::String(text)) => Ok(Value::String(text)),
            Some(Token::Word(word)) => {
                if self.peek() != Some(&Token::OpenParen) {
                    return Ok(Value::Word(word));
                }
                self.next();
                let mut args = Vec::new();
                loop {
                    match self.peek() {
                        Some(Token::CloseParen) => {
                            self.next();
                            return Ok(Value::Call { name: word, args });
                        }
                        Some(Token::String(_) | Token::Word(_)) => args.push(self.value()?),
                        _ => {
                            let other = self.next();
                            return Err(self.error(format!("expected `)`, found {other:?}")));
                        }
                    }
                }
            }
            other => Err(self.error(format!("expected a value, found {other:?}"))),
        }
    }
}

/// Nested objects are written `Key [Class] ["id"] {`, the values before the brace are the
/// class and the id.
fn object_from_header(header: Vec<Value>, entries: Vec<Entry>) -> Object {
    let mut object = Object {
        entries,
        ..Object::default()
    };
    for value in header {
        match value {
            Value::Word(class) if object.class.is_none() => object.class = Some(class),
            Value::String(id) if object.id.is_none() => object.id = Some(id),
            _ => {}
        }
    }
    object
}
