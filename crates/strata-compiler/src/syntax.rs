use crate::{Span, tokens::LexedSource};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxKind {
    CompilationUnit,
    NamespaceDeclaration,
    ImportDeclaration,
    ImportSelection,
    Binding,
    FunctionDeclaration,
    ParameterList,
    Parameter,
    Block,
    IfStatement,
    ElseClause,
    WhileStatement,
    ForStatement,
    ReturnStatement,
    BreakStatement,
    ContinueStatement,
    Assignment,
    BinaryExpression,
    TypeMembershipExpression,
    UnaryExpression,
    PostfixExpression,
    MemberExpression,
    IndexExpression,
    CallExpression,
    ArgumentList,
    Argument,
    GroupExpression,
    Name,
    ObjectName,
    Literal,
    TypeExpression,
    UnionType,
    PrefixType,
    AppliedType,
    FunctionType,
    Error,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxNode {
    pub kind: SyntaxKind,
    pub span: Span,
    pub token_range: std::ops::Range<usize>,
    pub children: Vec<SyntaxNode>,
}

impl SyntaxNode {
    pub(crate) fn new(
        kind: SyntaxKind,
        span: Span,
        token_range: std::ops::Range<usize>,
        children: Vec<Self>,
    ) -> Self {
        Self {
            kind,
            span,
            token_range,
            children,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SyntaxTree {
    pub lexed: LexedSource,
    pub root: SyntaxNode,
}

impl SyntaxTree {
    /// Produces a stable, whitespace-independent representation for parser goldens.
    #[must_use]
    pub fn normalized(&self) -> String {
        use std::fmt::Write as _;
        let mut output = String::new();
        Self::write_node(&self.root, 0, &mut output);
        output.push_str("tokens\n");
        for token in &self.lexed.tokens {
            let _ = writeln!(
                output,
                "  {:?} {}..{} {:?}",
                token.kind, token.span.start, token.span.end, token.text
            );
        }
        output.push_str("trivia\n");
        for trivia in &self.lexed.trivia {
            let _ = writeln!(
                output,
                "  {:?} {}..{} {:?}",
                trivia.kind, trivia.span.start, trivia.span.end, trivia.text
            );
        }
        output
    }

    fn write_node(node: &SyntaxNode, depth: usize, output: &mut String) {
        use std::fmt::Write as _;
        let _ = writeln!(
            output,
            "{}{:?} {}..{}",
            "  ".repeat(depth),
            node.kind,
            node.span.start,
            node.span.end
        );
        for child in &node.children {
            Self::write_node(child, depth + 1, output);
        }
    }
}
