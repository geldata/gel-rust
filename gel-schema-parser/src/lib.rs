mod ast;

use chumsky::{container::Seq, prelude::*, primitive::Todo};
use edgeql_parser::tokenizer::{Kind, Token, Tokenizer};

pub fn parse(source: &str) {
    let tokens: Vec<_> = Tokenizer::new(source)
        .validated_values()
        .collect::<Result<_, _>>()
        .unwrap();

    // let eof = tokens.last().unwrap();
    // let input =
    //     chumsky::input::Input::map(tokens.as_slice(), Span(eof.span), |t| (t, Span(t.span)));

    let res = root().parse(&tokens);

    for err in res.errors() {
        println!("err: {err:?}");
    }

    dbg!(res.output());
}

trait SchemaParser<'src, O>:
    Parser<'src, &'src [Token<'src>], O, chumsky::extra::Err<chumsky::error::Simple<'src, Token<'src>>>>
{
}

impl<'src, P, O> SchemaParser<'src, O> for P where
    P: Parser<
            'src,
            &'src [Token<'src>],
            O,
            chumsky::extra::Err<chumsky::error::Simple<'src, Token<'src>>>,
        >
{
}

fn root<'src>() -> impl SchemaParser<'src, ast::Root> {
    keyword("type")
        .ignore_then(path())
        .then_ignore(
            ctrl(Kind::OpenBrace)
                .then_ignore(ctrl(Kind::CloseBrace))
                .or_not(),
        )
        .then_ignore(ctrl(Kind::Semicolon).or_not())
        .map(|name| ast::Object { name })
        .repeated()
        .collect::<Vec<_>>()
        .map(|objects| ast::Root { objects })
        .then_ignore(end())
}

fn keyword<'src>(kw: &'static str) -> impl SchemaParser<'src, ()> {
    select! {
        Token { kind: Kind::Keyword(k), .. } if k.0 == kw => (),
    }
}

fn ident<'src>() -> impl SchemaParser<'src, String> {
    select! {
        Token { kind: Kind::Ident, text, .. } => text.into_owned(),
    }
}

fn ctrl<'src>(k: Kind) -> impl SchemaParser<'src, ()> {
    select! {
        Token { kind, .. } if kind == k => (),
    }
}

fn path<'src>() -> impl SchemaParser<'src, ast::Path> {
    ident()
        .separated_by(ctrl(Kind::Namespace))
        .at_least(1)
        .collect::<Vec<_>>()
        .map(|path: Vec<String>| ast::Path(path))
}

struct Span(edgeql_parser::position::Span);

impl chumsky::span::Span for Span {
    type Context = ();

    type Offset = u64;

    fn new(_context: Self::Context, range: std::ops::Range<Self::Offset>) -> Self {
        Span(edgeql_parser::position::Span {
            start: range.start,
            end: range.end,
        })
    }

    fn context(&self) -> Self::Context {
        ()
    }

    fn start(&self) -> Self::Offset {
        self.0.start
    }

    fn end(&self) -> Self::Offset {
        self.0.end
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_01() {
        super::parse(
            r#"
        type test::Foo {
            # required property foo: str;
        };

        type test::Bar {
            # required property bar: str;
        };
        "#,
        );
    }
}
