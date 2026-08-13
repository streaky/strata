use crate::Span;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Attachment {
    Detached,
    Left,
    Right,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Identifier,
    Number,
    String,
    TailString,
    BlockString,
    Newline,
    Indent,
    Dedent,
    Dot,
    Semicolon,
    Comma,
    Colon,
    Pipe,
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    OpenBrace,
    CloseBrace,
    Assign,
    Operator,
    Increment,
    Decrement,
    Eof,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub text: String,
    pub attachment: Attachment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriviaKind {
    Whitespace,
    LineComment,
    BlockComment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trivia {
    pub kind: TriviaKind,
    pub span: Span,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexedSource {
    pub tokens: Vec<Token>,
    pub trivia: Vec<Trivia>,
    pub logical_lines: Vec<(usize, String)>,
}
