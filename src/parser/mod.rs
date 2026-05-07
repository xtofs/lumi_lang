use chumsky::error::Rich;
use chumsky::input::{Stream, ValueInput};
use chumsky::prelude::*;
use chumsky::span::SimpleSpan;
use chumsky::Parser;
use logos::Logos;
use std::fmt;

use crate::ast::{Expr, Lit, MatchArm, Pattern};

// ── Tokens ────────────────────────────────────────────────────────────────────

#[derive(Logos, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Token {
    #[token("let")]
    Let,
    #[token("in")]
    In,
    #[token("if")]
    If,
    #[token("then")]
    Then,
    #[token("else")]
    Else,
    #[token("match")]
    Match,
    #[token("foreign")]
    Foreign,

    #[token("\\")]
    Backslash,
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,
    #[token("=")]
    Equals,
    #[token(",")]
    Comma,
    #[token("|")]
    Pipe,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    // `_` alone → Wildcard; `_foo` → Lower (logos: maximal munch)
    #[token("_")]
    Underscore,

    // NOTE: `-?[0-9]+` means `-42` is a single token.
    // If you ever add binary subtraction you'll need to split this.
    #[regex(r"-?[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    Int(i64),

    #[regex(r"[A-Z][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Upper(String),

    #[regex(r"[a-z][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Lower(String),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        s[1..s.len() - 1].to_string()
    })]
    Str(String),

    #[regex(r"[ \t\n\r]+", logos::skip)]
    #[regex(r"--[^\n]*", logos::skip, allow_greedy = true)] // line comments
    // #[error]
    Error,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Let => write!(f, "let"),
            Token::In => write!(f, "in"),
            Token::If => write!(f, "if"),
            Token::Then => write!(f, "then"),
            Token::Else => write!(f, "else"),
            Token::Match => write!(f, "match"),
            Token::Foreign => write!(f, "foreign"),
            Token::Backslash => write!(f, "\\"),
            Token::Arrow => write!(f, "->"),
            Token::FatArrow => write!(f, "=>"),
            Token::Equals => write!(f, "="),
            Token::Comma => write!(f, ","),
            Token::Pipe => write!(f, "|"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::Underscore => write!(f, "_"),
            Token::Int(n) => write!(f, "{n}"),
            Token::Upper(s) | Token::Lower(s) | Token::Str(s) => write!(f, "{s}"),
            Token::Error => write!(f, "<error>"),
        }
    }
}

// ── Lexer ─────────────────────────────────────────────────────────────────────

type Span = std::ops::Range<usize>;

pub fn lex(src: &str) -> (Vec<(Token, Span)>, Vec<String>) {
    let mut tokens = Vec::new();
    let mut errors = Vec::new();
    let mut lex = Token::lexer(src).spanned();
    while let Some((tok, span)) = lex.next() {
        match tok {
            Ok(tok) => tokens.push((tok, span)),
            Err(()) => errors.push(format!("unexpected character at {:?}", span)),
        }
    }
    (tokens, errors)
}

// ── Pattern parser ────────────────────────────────────────────────────────────
fn pattern<'src, I>() -> impl Parser<'src, I, Pattern, extra::Err<Rich<'src, Token>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = SimpleSpan>,
{
    recursive(|pat| {
        let wildcard = just(Token::Underscore).to(Pattern::Wildcard);
        let int_pat = select! {
            Token::Int(n) => Pattern::Lit(Lit::Int(n)),
        };
        let con_pat = select! {
            Token::Upper(s) => s,
        }
        .then(
            pat.separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .or_not()
                .map(|fs| fs.unwrap_or_default()),
        )
        .map(|(tag, fields)| Pattern::Con { tag, fields });
        let var_pat = select! {
            Token::Lower(s) => Pattern::Var(s),
        };
        choice((wildcard, int_pat, con_pat, var_pat))
    })
}
// ── Expression parser ─────────────────────────────────────────────────────────

pub fn expr<'src, I>() -> impl Parser<'src, I, Expr, extra::Err<Rich<'src, Token>>> + Clone
where
    I: ValueInput<'src, Token = Token, Span = SimpleSpan>,
{
    let pat = pattern();

    recursive(move |expr| {
        let int = select! {
            Token::Int(n) => Expr::Lit(Lit::Int(n)),
        };
        let var = select! {
            Token::Lower(s) => Expr::Var(s),
        };

        let paren = expr
            .clone()
            .delimited_by(just(Token::LParen), just(Token::RParen));

        let con = select! {
            Token::Upper(s) => s,
        }
        .then(
            expr.clone()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .or_not()
                .map(|fs| fs.unwrap_or_default()),
        )
        .map(|(tag, fields)| Expr::Con { tag, fields });

        let foreign = just(Token::Foreign)
            .ignore_then(select! {
                Token::Str(s) => s,
            })
            .then(
                expr.clone()
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(just(Token::LParen), just(Token::RParen)),
            )
            .map(|(name, args)| Expr::Foreign { name, args });

        let atom = choice((int, paren, foreign, con, var));

        let app = atom
            .clone()
            .then(atom.repeated().collect::<Vec<_>>())
            .map(|(f, xs)| {
                xs.into_iter()
                    .fold(f, |acc, x| Expr::App(Box::new(acc), Box::new(x)))
            });

        let let_ = just(Token::Let)
            .ignore_then(select! {
                Token::Lower(s) => s,
            })
            .then_ignore(just(Token::Equals))
            .then(expr.clone())
            .then_ignore(just(Token::In))
            .then(expr.clone())
            .map(|((name, value), body)| Expr::Let {
                name,
                value: Box::new(value),
                body: Box::new(body),
            });

        let lam = just(Token::Backslash)
            .ignore_then(select! {
                Token::Lower(s) => s,
            })
            .then_ignore(just(Token::Arrow))
            .then(expr.clone())
            .map(|(param, body)| Expr::Lam {
                param,
                body: Box::new(body),
            });

        let if_ = just(Token::If)
            .ignore_then(expr.clone())
            .then_ignore(just(Token::Then))
            .then(expr.clone())
            .then_ignore(just(Token::Else))
            .then(expr.clone())
            .map(|((cond, then_), else_)| Expr::If {
                cond: Box::new(cond),
                then_: Box::new(then_),
                else_: Box::new(else_),
            });

        let arm = just(Token::Pipe)
            .ignore_then(pat.clone())
            .then_ignore(just(Token::FatArrow))
            .then(expr.clone())
            .map(|(p, body)| MatchArm { pat: p, body });

        let match_ = just(Token::Match)
            .ignore_then(expr.clone())
            .then(
                arm.repeated()
                    .collect::<Vec<_>>()
                    .delimited_by(just(Token::LBrace), just(Token::RBrace)),
            )
            .map(|(scrutinee, arms)| Expr::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            });

        choice((let_, lam, if_, match_, app))
    })
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn parse(src: &str) -> (Option<Expr>, Vec<String>) {
    let (tokens, mut errors) = lex(src);
    let len = src.len();
    let stream = Stream::from_iter(tokens.into_iter().map(|(token, span)| (token, span.into())))
        .map((len..len).into(), |(token, span)| (token, span));
    let (ast, parse_errors) = expr().parse(stream).into_output_errors();
    errors.extend(parse_errors.into_iter().map(|e| e.to_string()));
    (ast, errors)
}
