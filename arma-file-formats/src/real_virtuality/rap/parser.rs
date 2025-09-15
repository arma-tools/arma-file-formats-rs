use crate::{
    errors::AffError,
    real_virtuality::rap::{CfgClass, CfgEntry, CfgProperty, CfgValue},
};
use ariadne::{Color, Fmt, Label, Report, ReportKind, Source};
use chumsky::{prelude::*, stream::Stream};
use std::{fmt, io::Cursor};

pub type Span = std::ops::Range<usize>;

pub type Spanned<T> = (T, Span);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Token {
    Num(String),
    Str(String),
    Ctrl(char),
    Ident(String),
    Bool(bool),
    Class,
    Delete,
    StringConcat,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Str(s) | Self::Ident(s) | Self::Num(s) => write!(f, "{s}"),
            Self::Ctrl(c) => write!(f, "{c}"),
            Self::Class => write!(f, "class"),
            Self::Delete => write!(f, "delete"),
            Self::StringConcat => write!(f, " \\n "),
            Self::Bool(b) => write!(f, "{b}"),
        }
    }
}

fn lexer() -> impl Parser<char, Vec<(Token, Span)>, Error = Simple<char>> {
    // frac/exponent/number
    let frac = just('.').chain(text::digits(10));

    let exp = just('e')
        .or(just('E'))
        .chain(just('+').or(just('-')).or_not())
        .chain::<char, _, _>(text::digits(10));

    let num = just('-')
        .or_not()
        .chain::<char, _, _>(text::int(10))
        .chain::<char, _, _>(frac.or_not().flatten())
        .chain::<char, _, _>(exp.or_not().flatten())
        .collect::<String>()
        .map(|str: String| {
            if str.parse::<f32>().is_ok() {
                Token::Num(str)
            } else {
                Token::Str(str)
            }
        });

    // Strings are "concated" by " \n "
    let string_concat = just("\\n").padded().map(|_| Token::StringConcat);

    // single string
    let str_ = just('"')
        .ignore_then(filter(|c| *c != '"').or(just("\"\"").to('\"')).repeated())
        .then_ignore(just('"'))
        .collect::<String>()
        .map(Token::Str);

    // strings concated by " \n "
    let concat_stings = str_
        .separated_by(string_concat)
        .at_least(1)
        .map(|tokens| {
            Token::Str(
                tokens
                    .iter()
                    .map(|t| {
                        if let Token::Str(s) = t {
                            s.clone()
                        } else {
                            String::new()
                        }
                    })
                    .collect::<Vec<String>>()
                    .join("\n"),
            )
        })
        .or(str_);

    // control characters
    let ctrl = one_of("[]{};,:=").map(Token::Ctrl);

    // identifiers and keywords
    let ident = text::ident().map(|ident: String| match ident.to_lowercase().as_str() {
        "class" => Token::Class,
        "delete" => Token::Delete,
        "true" => Token::Bool(true),
        "false" => Token::Bool(false),
        _ => Token::Ident(ident),
    });

    let token = ident
        .or(ctrl)
        .or(concat_stings)
        .or(num)
        .recover_with(skip_then_retry_until([]));

    let comment = just("//")
        .or(just("#"))
        .then(take_until(just('\n')))
        .padded();
    //let define = just("#").then(take_until(just('\n'))).padded();
    let ml_comment = just("/*").then(take_until(just("*/"))).padded();

    token
        .map_with_span(|tok, span| (tok, span))
        .padded_by(comment.repeated())
        .padded_by(ml_comment.repeated())
        // .padded_by(define.repeated())
        .padded()
        .repeated()
}

#[derive(Debug, PartialEq, Clone)]
enum EntryExpr {
    Prop(String, Box<Spanned<ValueExpr>>),
    Class(String, Option<String>, Vec<Spanned<EntryExpr>>),
    Extern(String),
    Delete(String),
}

impl From<EntryExpr> for CfgEntry {
    fn from(val: EntryExpr) -> Self {
        match val {
            EntryExpr::Prop(name, value) => Self::Property(CfgProperty {
                name,
                value: value.0.into(),
            }),
            EntryExpr::Class(name, parent, entries) => Self::Class(CfgClass {
                name,
                parent,
                entries: entries.into_iter().map(|e| e.0.into()).collect(),
            }),
            EntryExpr::Extern(e) => Self::Extern(e),
            EntryExpr::Delete(d) => Self::Delete(d),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
enum ValueExpr {
    Long(i32),
    Float(f32),
    Str(String),
    Array(Vec<Spanned<Self>>),
}

impl From<ValueExpr> for CfgValue {
    fn from(val: ValueExpr) -> Self {
        match val {
            ValueExpr::Long(l) => Self::Long(l),
            ValueExpr::Float(f) => Self::Float(f),
            ValueExpr::Str(s) => Self::String(s),
            ValueExpr::Array(a) => Self::Array(a.into_iter().map(|e| e.0.into()).collect()),
        }
    }
}

fn entry_parser() -> impl Parser<Token, Vec<Spanned<EntryExpr>>, Error = Simple<Token>> + Clone {
    let val = select! {
        Token::Num(n) => {
            let num = n.parse::<f32>().unwrap();
            if num.fract() == 0.0 {
                ValueExpr::Long(num as i32)
            } else{
                ValueExpr::Float(num)

            }
        },
        Token::Str(s) => ValueExpr::Str(s),
        Token::Bool(b) => ValueExpr::Long(i32::from(b)),
    }
    .map_with_span(|ident, span| (ident, span))
    .labelled("value");

    let ident = select! { Token::Ident(ident) => ident }.labelled("identifier");

    let arr_vals = recursive(|arr_val| {
        val.or(arr_val)
            .separated_by(just(Token::Ctrl(',')))
            .allow_trailing()
            .delimited_by(just(Token::Ctrl('{')), just(Token::Ctrl('}')))
            .map_with_span(|s, v| (ValueExpr::Array(s), v))
    });

    let prop = ident
        .then_ignore(just(Token::Ctrl('=')))
        .then(val)
        .then_ignore(just(Token::Ctrl(';')))
        .map_with_span(|(name, value), span| (EntryExpr::Prop(name, Box::new(value)), span));
    // .recover_with(skip_then_retry_until([Token::Ctrl(';')]).consume_end())

    let prop_arr = ident
        .then_ignore(just(Token::Ctrl('[')))
        .then_ignore(just(Token::Ctrl(']')))
        .then_ignore(just(Token::Ctrl('=')))
        .then(arr_vals)
        .then_ignore(just(Token::Ctrl(';')))
        .map_with_span(|(name, value), span| (EntryExpr::Prop(name, Box::new(value)), span));
    // .recover_with(nested_delimiters(
    //     Token::Ctrl('{'),
    //     Token::Ctrl('}'),
    //     [
    //         (Token::Ctrl('('), Token::Ctrl(')')),
    //         (Token::Ctrl('['), Token::Ctrl(']')),
    //     ],
    //     |span| (Expr2::Error, span),
    // ))
    // .recover_with(skip_then_retry_until([Token::Ctrl(';')]).consume_end());

    let extern_class = just(Token::Class)
        .ignore_then(ident)
        .then_ignore(just(Token::Ctrl(';')))
        .map_with_span(|name, span| (EntryExpr::Extern(name), span));
    // .recover_with(skip_then_retry_until([Token::Ctrl(';')]).consume_end());

    let del_class = just(Token::Delete)
        .ignore_then(ident)
        .then_ignore(just(Token::Ctrl(';')))
        .map_with_span(|name, span| (EntryExpr::Delete(name), span));
    // .recover_with(skip_then_retry_until([Token::Ctrl(';')]).consume_end());

    let parent = just(Token::Ctrl(':')).ignore_then(ident).or_not();

    let entry = prop
        .clone()
        .or(prop_arr.clone())
        .or(extern_class)
        .or(del_class);

    let class = recursive(|cl_pr| {
        just(Token::Class)
            .ignore_then(ident)
            .then(parent)
            .then_ignore(just(Token::Ctrl('{')))
            .then(
                cl_pr.or(entry.clone()).repeated(), //.map_with_span(|entry, span| (entry, span))),
            )
            .then_ignore(just(Token::Ctrl('}')))
            .then_ignore(just(Token::Ctrl(';')))
            .map_with_span(|((name, parent), entries), span| {
                (EntryExpr::Class(name, parent, entries), span)
            })
        // .recover_with(nested_delimiters(
        //     Token::Ctrl('{'),
        //     Token::Ctrl('}'),
        //     [
        //         (Token::Ctrl('('), Token::Ctrl(')')),
        //         (Token::Ctrl('['), Token::Ctrl(']')),
        //     ],
        //     |span| (Expr2::Error, span),
        // ))
        // .recover_with(skip_then_retry_until([Token::Ctrl(';')]).consume_end())
    });

    //choice((entry)).repeated().then_ignore(end())
    class.or(entry).repeated().then_ignore(end())
}

pub fn parse(src: &str) -> Result<Vec<CfgEntry>, AffError> {
    let (tokens, errs) = lexer().parse_recovery(src);
    // dbg!(&tokens);
    // dbg!(errs.clone());

    let parse_errs = if let Some(tokens) = tokens {
        //dbg!(&tokens);
        let len = src.chars().count();
        #[allow(clippy::range_plus_one)]
        let (ast, parse_errs) =
            entry_parser().parse_recovery(Stream::from_iter(len..len + 1, tokens.into_iter()));

        //dbg!(ast);
        if let Some(funcs) = ast.filter(|_| errs.len() + parse_errs.len() == 0) {
            return Ok(funcs.into_iter().map(|e| e.0.into()).collect());
        }

        parse_errs
    } else {
        Vec::new()
    };

    let errs_str: Vec<String> = errs
        .into_iter()
        .map(|e| e.map(|c| c.to_string()))
        .chain(parse_errs.into_iter().map(|e| e.map(|tok| tok.to_string())))
        .map(|e| {
            let report = Report::build(ReportKind::Error, (), e.span().start);

            let report = match e.reason() {
                chumsky::error::SimpleReason::Unclosed { span, delimiter } => report
                    .with_message(format!(
                        "Unclosed delimiter {}",
                        delimiter.fg(Color::Yellow)
                    ))
                    .with_label(
                        Label::new(span.clone())
                            .with_message(format!(
                                "Unclosed delimiter {}",
                                delimiter.fg(Color::Yellow)
                            ))
                            .with_color(Color::Yellow),
                    )
                    .with_label(
                        Label::new(e.span())
                            .with_message(format!(
                                "Must be closed before this {}",
                                e.found()
                                    .unwrap_or(&"end of file".to_string())
                                    .fg(Color::Red)
                            ))
                            .with_color(Color::Red),
                    ),
                chumsky::error::SimpleReason::Unexpected => report
                    .with_message(format!(
                        "{}, expected '{}'",
                        if e.found().is_some() {
                            "Unexpected token in input"
                        } else {
                            "Unexpected end of input"
                        },
                        if std::iter::ExactSizeIterator::len(&e.expected()) == 0 {
                            e.label()
                                .map_or_else(|| "something else".to_string(), ToString::to_string)
                        } else {
                            e.expected()
                                .map(|expected| {
                                    expected.as_ref().map_or_else(
                                        || "end of input".to_string(),
                                        ToString::to_string,
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        }
                    ))
                    .with_label(
                        Label::new(e.span())
                            .with_message(format!(
                                "Unexpected token {}",
                                e.found()
                                    .unwrap_or(&"end of file".to_string())
                                    .fg(Color::Red)
                            ))
                            .with_color(Color::Red),
                    ),
                chumsky::error::SimpleReason::Custom(msg) => report.with_message(msg).with_label(
                    Label::new(e.span())
                        .with_message(format!("{}", msg.fg(Color::Red)))
                        .with_color(Color::Red),
                ),
            };

            let mut err_buf = Vec::new();
            let err_cur = Cursor::new(&mut err_buf);
            if report.finish().write(Source::from(&src), err_cur).is_ok() {
                String::from_utf8(err_buf).unwrap_or_default()
            } else {
                String::new()
            }
        })
        .collect();

    Err(AffError::ParseError(errs_str.join("\n")))
}
