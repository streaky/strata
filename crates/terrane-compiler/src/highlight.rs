use crate::lexer::lex_recovering;
use crate::parser::parse;
use crate::syntax::{SyntaxKind, SyntaxNode};
use crate::tokens::{Token, TokenKind, TriviaKind};
use crate::{Diagnostic, SourceFile, Span};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HighlightKind {
    Comment,
    Keyword,
    Number,
    String,
    Operator,
    Namespace,
    Type,
    Function,
    Parameter,
    Property,
    Variable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Highlight {
    pub span: Span,
    pub kind: HighlightKind,
    pub declaration: bool,
}

#[derive(Clone, Debug)]
pub struct HighlightOutput {
    pub highlights: Vec<Highlight>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Classifies source spans using the recovering compiler lexer and syntax tree.
#[must_use]
pub fn highlight(source: &SourceFile) -> HighlightOutput {
    let lexical = lex_recovering(source);
    let parsed = parse(source, lexical.lexed);
    let mut diagnostics = lexical.diagnostics;
    diagnostics.extend(parsed.diagnostics);

    let tokens = &parsed.tree.lexed.tokens;
    let mut classified = tokens.iter().map(base_classification).collect::<Vec<_>>();
    classify_node(&parsed.tree.root, tokens, &mut classified);

    let mut highlights = parsed
        .tree
        .lexed
        .trivia
        .iter()
        .filter(|trivia| {
            matches!(
                trivia.kind,
                TriviaKind::LineComment | TriviaKind::BlockComment
            )
        })
        .map(|trivia| Highlight {
            span: trivia.span,
            kind: HighlightKind::Comment,
            declaration: false,
        })
        .collect::<Vec<_>>();

    highlights.extend(
        tokens
            .iter()
            .zip(classified)
            .filter_map(|(token, classification)| {
                classification.map(|(kind, declaration)| Highlight {
                    span: token.span,
                    kind,
                    declaration,
                })
            }),
    );
    highlights.retain(|highlight| highlight.span.start < highlight.span.end);
    highlights.sort_by_key(|highlight| (highlight.span.start, highlight.span.end));

    HighlightOutput {
        highlights,
        diagnostics,
    }
}

fn base_classification(token: &Token) -> Option<(HighlightKind, bool)> {
    match token.kind {
        TokenKind::Number => Some((HighlightKind::Number, false)),
        TokenKind::String | TokenKind::TailString | TokenKind::BlockString => {
            Some((HighlightKind::String, false))
        }
        TokenKind::Assign
        | TokenKind::Operator
        | TokenKind::Increment
        | TokenKind::Decrement
        | TokenKind::Pipe => Some((HighlightKind::Operator, false)),
        TokenKind::Identifier if matches!(token.text.as_str(), "true" | "false") => {
            Some((HighlightKind::Keyword, false))
        }
        TokenKind::Identifier if is_keyword(&token.text) => Some((HighlightKind::Keyword, false)),
        TokenKind::Identifier => Some((HighlightKind::Variable, false)),
        _ => None,
    }
}

fn classify_node(
    node: &SyntaxNode,
    tokens: &[Token],
    classified: &mut [Option<(HighlightKind, bool)>],
) {
    for child in &node.children {
        classify_node(child, tokens, classified);
    }
    match node.kind {
        SyntaxKind::NamespaceDeclaration | SyntaxKind::NamespacePath => {
            classify_names(node, tokens, classified, HighlightKind::Namespace, false);
        }
        SyntaxKind::TypeExpression => {
            classify_names(node, tokens, classified, HighlightKind::Type, false);
        }
        SyntaxKind::Parameter => {
            if let Some(name) = node
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Name)
            {
                classify_names(name, tokens, classified, HighlightKind::Parameter, true);
            }
        }
        SyntaxKind::FunctionDeclaration => {
            if let Some(name) = node
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Name)
            {
                classify_names(name, tokens, classified, HighlightKind::Function, true);
            }
        }
        SyntaxKind::Binding => {
            if let Some(name) = node
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Name)
            {
                classify_names(name, tokens, classified, HighlightKind::Variable, true);
            }
        }
        SyntaxKind::MemberExpression => {
            if let Some(name) = node
                .children
                .last()
                .filter(|child| child.kind == SyntaxKind::Name)
            {
                classify_names(name, tokens, classified, HighlightKind::Property, false);
            }
        }
        SyntaxKind::CallExpression => {
            if let Some(callee) = node.children.first() {
                if let Some(index) = last_name_token(callee, tokens) {
                    classified[index] = Some((HighlightKind::Function, false));
                }
            }
        }
        _ => {}
    }
}

fn classify_names(
    node: &SyntaxNode,
    tokens: &[Token],
    classified: &mut [Option<(HighlightKind, bool)>],
    kind: HighlightKind,
    declaration: bool,
) {
    for index in node.token_range.clone() {
        if tokens[index].kind == TokenKind::Identifier && !is_keyword(&tokens[index].text) {
            classified[index] = Some((kind, declaration));
        }
    }
}

fn last_name_token(node: &SyntaxNode, tokens: &[Token]) -> Option<usize> {
    node.token_range.clone().rev().find(|index| {
        tokens[*index].kind == TokenKind::Identifier && !is_keyword(&tokens[*index].text)
    })
}

fn is_keyword(text: &str) -> bool {
    matches!(
        text,
        "namespace"
            | "from"
            | "import"
            | "as"
            | "function"
            | "public"
            | "private"
            | "protected"
            | "global"
            | "constant"
            | "static"
            | "async"
            | "mutating"
            | "throws"
            | "if"
            | "else"
            | "while"
            | "for"
            | "in"
            | "return"
            | "break"
            | "continue"
            | "and"
            | "or"
            | "not"
            | "is"
            | "a"
            | "ref"
            | "move"
            | "await"
            | "of"
            | "to"
            | "true"
            | "false"
            | "class"
            | "try"
            | "throw"
            | "yield"
            | "match"
            | "unsafe"
            | "rust"
            | "label"
            | "goto"
            | "when"
            | "use"
            | "catch"
            | "finally"
            | "case"
    )
}
