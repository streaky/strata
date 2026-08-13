use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxTree};
use crate::tokens::{Attachment, LexedSource, Token, TokenKind};
use crate::{Diagnostic, SourceFile, Span};

/// Parses lexer output into a lossless, formatter-ready syntax tree.
///
/// # Errors
///
/// Returns source-oriented syntax diagnostics after recovering at layout boundaries.
pub fn parse(source: &SourceFile, lexed: LexedSource) -> Result<SyntaxTree, Vec<Diagnostic>> {
    let mut parser = Parser {
        source,
        tokens: &lexed.tokens,
        position: 0,
        semicolon_boundary: false,
        diagnostics: Vec::new(),
    };
    let root = parser.parse_compilation_unit();
    if parser.diagnostics.is_empty() {
        Ok(SyntaxTree { lexed, root })
    } else {
        Err(parser.diagnostics)
    }
}

struct Parser<'source> {
    source: &'source SourceFile,
    tokens: &'source [Token],
    position: usize,
    semicolon_boundary: bool,
    diagnostics: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn parse_compilation_unit(&mut self) -> SyntaxNode {
        let start = self.position;
        let mut children = Vec::new();
        self.skip_newlines();
        while !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Dedent) {
                self.error_here("S1001", "unexpected dedent");
                self.bump();
            } else {
                children.push(self.parse_statement());
                self.finish_statement();
            }
            self.skip_newlines();
        }
        self.node(SyntaxKind::CompilationUnit, start, self.position, children)
    }

    fn parse_statement(&mut self) -> SyntaxNode {
        match self.text() {
            "namespace" => self.parse_namespace(),
            "function" | "public" | "private" | "protected" | "static" | "async" | "mutating"
            | "throws"
                if self.looks_like_function_declaration() =>
            {
                self.parse_function()
            }
            "if" => self.parse_if(),
            "while" => self.parse_while(),
            "for" => self.parse_for(),
            "return" => self.parse_simple_value_statement(SyntaxKind::ReturnStatement, false),
            "break" => self.parse_bare_statement(SyntaxKind::BreakStatement),
            "continue" => self.parse_bare_statement(SyntaxKind::ContinueStatement),
            "from" => self.parse_import_declaration(),
            "import" => self.parse_import_selection(),
            "class" | "try" | "throw" | "yield" | "match" | "unsafe" | "rust" | "label"
            | "goto" | "when" | "use" => self.parse_unsupported(),
            _ if self.looks_like_binding() => self.parse_binding(),
            _ => self.parse_expression_statement(),
        }
    }

    fn parse_namespace(&mut self) -> SyntaxNode {
        let start = self.position;
        self.bump();
        let mut children = Vec::new();
        while !self.at_line_end() {
            if self.at(TokenKind::Identifier) {
                children.push(self.leaf(SyntaxKind::Name));
            } else {
                self.error_here("S1002", "expected a namespace component");
                self.bump();
            }
        }
        if children.is_empty() {
            self.error_at(start, "S1002", "namespace declaration requires a path");
        }
        self.node(
            SyntaxKind::NamespaceDeclaration,
            start,
            self.position,
            children,
        )
    }

    fn parse_import_declaration(&mut self) -> SyntaxNode {
        let start = self.position;
        self.bump();
        let mut children = vec![self.parse_namespace_path()];
        self.expect_text("import", "S1026", "expected `import` after namespace path");
        if self.at_line_end() {
            self.error_here("S1026", "expected an object name after `import`");
        } else {
            loop {
                children.push(self.parse_object_import());
                if !self.eat(TokenKind::Comma) {
                    break;
                }
                if self.at_line_end() {
                    self.error_here("S1026", "expected an object name after `,`");
                    break;
                }
            }
        }
        self.node(
            SyntaxKind::ImportDeclaration,
            start,
            self.position,
            children,
        )
    }

    fn parse_namespace_path(&mut self) -> SyntaxNode {
        let start = self.position;
        let mut children = Vec::new();
        if self.at_text("/") {
            self.bump();
        } else {
            while self.at(TokenKind::Dot) && self.peek_kind(1) == Some(TokenKind::Dot) {
                self.bump();
                self.bump();
            }
        }
        while !self.at_text("import") && !self.at_line_end() {
            if self.at(TokenKind::Identifier) {
                children.push(self.leaf(SyntaxKind::Name));
            } else {
                self.error_here("S1026", "expected a namespace path component");
                self.bump();
            }
        }
        if children.is_empty() {
            self.error_at(start, "S1026", "expected a namespace path after `from`");
        }
        self.node(SyntaxKind::NamespacePath, start, self.position, children)
    }

    fn parse_object_import(&mut self) -> SyntaxNode {
        let start = self.position;
        let mut children = Vec::new();
        children.push(self.parse_object_name("S1026", "expected an object name"));
        if self.eat_text("as") {
            let alias_start = self.position.saturating_sub(1);
            let alias = self.parse_object_name("S1026", "expected an object alias after `as`");
            children.push(self.node(
                SyntaxKind::ImportAlias,
                alias_start,
                self.position,
                vec![alias],
            ));
        }
        self.node(SyntaxKind::ObjectImport, start, self.position, children)
    }

    fn parse_import_selection(&mut self) -> SyntaxNode {
        let start = self.position;
        self.bump();
        self.expect_text("with", "S1027", "expected `with` after `import`");
        let importer = self.parse_object_name("S1027", "expected an importer object after `with`");
        self.node(
            SyntaxKind::ImportSelection,
            start,
            self.position,
            vec![importer],
        )
    }

    fn parse_object_name(&mut self, code: &'static str, message: &str) -> SyntaxNode {
        let start = self.position;
        if !self.eat(TokenKind::Dot) {
            self.error_here(code, message);
            if !self.at_line_end() {
                self.bump();
            }
            return self.node(SyntaxKind::Error, start, self.position, Vec::new());
        }
        if self.at(TokenKind::Identifier) {
            let name = self.leaf(SyntaxKind::Name);
            self.node(SyntaxKind::ObjectName, start, self.position, vec![name])
        } else {
            self.error_here(code, message);
            self.node(SyntaxKind::Error, start, self.position, Vec::new())
        }
    }

    fn parse_binding(&mut self) -> SyntaxNode {
        let start = self.position;
        while matches!(
            self.text(),
            "public" | "private" | "protected" | "global" | "constant"
        ) {
            self.bump();
        }
        let mut children = Vec::new();
        if self.at(TokenKind::Identifier) {
            children.push(self.leaf(SyntaxKind::Name));
        } else {
            self.error_here("S1003", "expected a binding name");
        }
        if !self.at(TokenKind::Assign) && !self.at_line_end() {
            children.push(self.parse_type_expression());
        }
        if self.eat(TokenKind::Assign) {
            if self.at_line_end() {
                self.error_here("S1004", "expected an initializer after `=`");
            } else {
                children.push(self.parse_expression(0, true));
            }
        }
        self.node(SyntaxKind::Binding, start, self.position, children)
    }

    fn parse_function(&mut self) -> SyntaxNode {
        let start = self.position;
        while !self.at_text("function") && !self.at_line_end() {
            self.bump();
        }
        self.expect_text("function", "S1005", "expected `function`");
        let mut children = Vec::new();
        if self.at(TokenKind::Identifier) && !self.at_text("from") && !self.at_text("to") {
            children.push(self.leaf(SyntaxKind::Name));
            if !self.at(TokenKind::Semicolon) && !self.at_line_end() {
                children.push(self.parse_type_expression());
            }
        }
        if self.eat(TokenKind::Semicolon) {
            children.push(self.parse_parameter_list());
        }
        if !self.at(TokenKind::Newline) {
            self.error_here("S1006", "unexpected content in function header");
            self.recover_line();
        }
        children.push(self.parse_block());
        self.node(
            SyntaxKind::FunctionDeclaration,
            start,
            self.position,
            children,
        )
    }

    fn parse_parameter_list(&mut self) -> SyntaxNode {
        let start = self.position;
        let mut children = Vec::new();
        while !self.at_line_end() {
            let parameter_start = self.position;
            if self.at(TokenKind::Identifier) {
                let mut parts = vec![self.leaf(SyntaxKind::Name)];
                if !self.at(TokenKind::Assign) && !self.at(TokenKind::Comma) && !self.at_line_end()
                {
                    parts.push(self.parse_type_expression());
                }
                if self.eat(TokenKind::Assign) {
                    parts.push(self.parse_expression(0, false));
                }
                children.push(self.node(
                    SyntaxKind::Parameter,
                    parameter_start,
                    self.position,
                    parts,
                ));
            } else {
                self.error_here("S1007", "expected a parameter name");
                self.recover_to_comma_or_line();
            }
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.node(SyntaxKind::ParameterList, start, self.position, children)
    }

    fn parse_if(&mut self) -> SyntaxNode {
        let start = self.position;
        self.bump();
        let mut children = vec![self.require_expression("if condition")];
        children.push(self.parse_block());
        while self.at_text("else") {
            let clause_start = self.position;
            self.bump();
            let mut clause = Vec::new();
            if self.eat_text("if") {
                clause.push(self.require_expression("else-if condition"));
            }
            clause.push(self.parse_block());
            children.push(self.node(SyntaxKind::ElseClause, clause_start, self.position, clause));
        }
        self.node(SyntaxKind::IfStatement, start, self.position, children)
    }

    fn parse_while(&mut self) -> SyntaxNode {
        let start = self.position;
        self.bump();
        let condition = self.require_expression("while condition");
        let block = self.parse_block();
        self.node(
            SyntaxKind::WhileStatement,
            start,
            self.position,
            vec![condition, block],
        )
    }

    fn parse_for(&mut self) -> SyntaxNode {
        let start = self.position;
        self.bump();
        let mut children = Vec::new();
        if self.line_has_semicolons(2) {
            children.push(self.parse_for_clause());
            self.expect(
                TokenKind::Semicolon,
                "S1008",
                "expected `;` after for initializer",
            );
            children.push(self.parse_for_expression());
            self.expect(
                TokenKind::Semicolon,
                "S1008",
                "expected `;` after for condition",
            );
            children.push(self.parse_for_clause());
            if self.at(TokenKind::Semicolon) {
                self.error_here(
                    "S1016",
                    "calls inside three-clause `for` clauses must be parenthesized",
                );
                self.recover_line();
            }
        } else {
            children.push(self.require_expression("for target"));
            self.expect_text("in", "S1009", "expected `in` in collection for");
            children.push(self.require_expression("for collection"));
        }
        children.push(self.parse_block());
        self.node(SyntaxKind::ForStatement, start, self.position, children)
    }

    fn parse_for_clause(&mut self) -> SyntaxNode {
        let start = self.position;
        let left = self.parse_for_expression();
        if self.eat(TokenKind::Assign) {
            let right = self.parse_for_expression();
            self.node(
                SyntaxKind::Assignment,
                start,
                self.position,
                vec![left, right],
            )
        } else {
            left
        }
    }

    fn parse_for_expression(&mut self) -> SyntaxNode {
        self.semicolon_boundary = true;
        let expression = self.parse_expression(0, false);
        self.semicolon_boundary = false;
        expression
    }

    fn parse_simple_value_statement(&mut self, kind: SyntaxKind, required: bool) -> SyntaxNode {
        let start = self.position;
        self.bump();
        let mut children = Vec::new();
        if !self.at_line_end() {
            children.push(self.parse_expression(0, true));
        } else if required {
            self.error_here("S1010", "expected an expression");
        }
        self.node(kind, start, self.position, children)
    }

    fn parse_bare_statement(&mut self, kind: SyntaxKind) -> SyntaxNode {
        let start = self.position;
        self.bump();
        if !self.at_line_end() {
            self.error_here("S1011", "this statement does not take a value");
            self.recover_line();
        }
        self.node(kind, start, self.position, Vec::new())
    }

    fn parse_unsupported(&mut self) -> SyntaxNode {
        let start = self.position;
        let feature = self.text().to_owned();
        self.diagnostics.push(Diagnostic::error(
            "S1090",
            format!("`{feature}` syntax is reserved but not supported by this compiler milestone"),
            self.current().span,
        ));
        self.recover_line();
        if self.at(TokenKind::Newline) && self.peek_kind(1) == Some(TokenKind::Indent) {
            self.bump();
            self.bump();
            self.recover_nested_block();
        }
        self.node(SyntaxKind::Unsupported, start, self.position, Vec::new())
    }

    fn parse_expression_statement(&mut self) -> SyntaxNode {
        let start = self.position;
        let left = self.parse_expression(0, true);
        if self.eat(TokenKind::Assign) {
            let right = self.require_expression("assignment value");
            self.node(
                SyntaxKind::Assignment,
                start,
                self.position,
                vec![left, right],
            )
        } else {
            left
        }
    }

    fn parse_expression(&mut self, minimum: u8, allow_call: bool) -> SyntaxNode {
        let start = self.position;
        let mut left = self.parse_prefix(allow_call);
        loop {
            if minimum <= 3
                && self.at_text("is")
                && self.peek_text(1) == Some("a")
                && self.type_starts_at(2)
            {
                self.bump();
                self.bump();
                let type_expression = self.parse_type_expression();
                left = self.node(
                    SyntaxKind::TypeMembershipExpression,
                    start,
                    self.position,
                    vec![left, type_expression],
                );
                continue;
            }
            if let Some(precedence) = self.binary_precedence() {
                if precedence < minimum {
                    break;
                }
                if self.at_text("==") && self.peek_kind(1) == Some(TokenKind::Assign) {
                    self.error_here(
                        "S1091",
                        "`===` is unsupported; use `==` for equality or `is` for identity",
                    );
                    self.bump();
                    self.bump();
                    self.recover_expression();
                    break;
                }
                let operator = self.text().to_owned();
                self.bump();
                let right = self.parse_expression(precedence + 1, allow_call);
                left = self.node(
                    SyntaxKind::BinaryExpression,
                    start,
                    self.position,
                    vec![left, right],
                );
                if Self::is_comparison(&operator) && self.binary_precedence() == Some(precedence) {
                    self.error_here(
                        "S1012",
                        "comparisons do not chain; join comparisons with `and`",
                    );
                    self.recover_expression();
                    break;
                }
                continue;
            }
            break;
        }
        left
    }

    fn parse_prefix(&mut self, allow_call: bool) -> SyntaxNode {
        if matches!(self.text(), "not" | "ref" | "move" | "await")
            || (self.at(TokenKind::Operator) && matches!(self.text(), "-" | "~"))
        {
            let start = self.position;
            let restricted = matches!(self.text(), "ref" | "move" | "await");
            self.bump();
            let operand = if restricted {
                self.parse_postfix(false)
            } else {
                self.parse_prefix(allow_call)
            };
            return self.node(
                SyntaxKind::UnaryExpression,
                start,
                self.position,
                vec![operand],
            );
        }
        self.parse_postfix(allow_call)
    }

    fn parse_postfix(&mut self, allow_call: bool) -> SyntaxNode {
        let start = self.position;
        let mut value = self.parse_primary(allow_call);
        loop {
            if self.at(TokenKind::Dot) {
                if self.current().attachment != Attachment::Both {
                    self.error_here(
                        "S1013",
                        "member access requires no whitespace before the dot; write `value.member`",
                    );
                }
                self.bump();
                if self.at(TokenKind::Identifier) {
                    let name = self.leaf(SyntaxKind::Name);
                    value = self.node(
                        SyntaxKind::MemberExpression,
                        start,
                        self.position,
                        vec![value, name],
                    );
                } else {
                    self.error_here("S1014", "expected a member name after `.`");
                }
            } else if self.eat(TokenKind::OpenBracket) {
                let index = self.require_expression("index");
                self.expect(TokenKind::CloseBracket, "S1015", "expected `]` after index");
                value = self.node(
                    SyntaxKind::IndexExpression,
                    start,
                    self.position,
                    vec![value, index],
                );
            } else if self.at(TokenKind::Increment) || self.at(TokenKind::Decrement) {
                self.bump();
                value = self.node(
                    SyntaxKind::PostfixExpression,
                    start,
                    self.position,
                    vec![value],
                );
            } else {
                break;
            }
        }
        if self.at(TokenKind::Semicolon) {
            if allow_call {
                self.bump();
                let arguments = self.parse_argument_list();
                value = self.node(
                    SyntaxKind::CallExpression,
                    start,
                    self.position,
                    vec![value, arguments],
                );
            } else if !self.semicolon_boundary {
                self.error_here("S1016", "nested calls must be parenthesized");
                self.recover_expression();
            }
        }
        value
    }

    fn parse_argument_list(&mut self) -> SyntaxNode {
        let start = self.position;
        let mut children = Vec::new();
        while !self.at_expression_end() {
            let argument_start = self.position;
            let mut parts = Vec::new();
            if self.at(TokenKind::Identifier) && self.peek_kind(1) == Some(TokenKind::Assign) {
                parts.push(self.leaf(SyntaxKind::Name));
                self.bump();
            }
            parts.push(self.parse_expression(0, false));
            children.push(self.node(SyntaxKind::Argument, argument_start, self.position, parts));
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.node(SyntaxKind::ArgumentList, start, self.position, children)
    }

    fn parse_primary(&mut self, allow_call: bool) -> SyntaxNode {
        match self.current().kind {
            TokenKind::Identifier => self.leaf(SyntaxKind::Name),
            TokenKind::Number
            | TokenKind::String
            | TokenKind::TailString
            | TokenKind::BlockString => self.leaf(SyntaxKind::Literal),
            TokenKind::Dot => {
                let start = self.position;
                self.bump();
                if self.at(TokenKind::Identifier) {
                    let name = self.leaf(SyntaxKind::Name);
                    self.node(SyntaxKind::ObjectName, start, self.position, vec![name])
                } else {
                    self.error_here("S1017", "expected an object name after `.`");
                    self.node(SyntaxKind::Error, start, self.position, Vec::new())
                }
            }
            TokenKind::OpenParen => {
                let start = self.position;
                self.bump();
                let expression = self.parse_expression(0, true);
                self.expect(
                    TokenKind::CloseParen,
                    "S1018",
                    "expected `)` after grouped expression",
                );
                self.node(
                    SyntaxKind::GroupExpression,
                    start,
                    self.position,
                    vec![expression],
                )
            }
            _ => {
                let start = self.position;
                self.error_here("S1019", "expected an expression");
                if !self.at_expression_end() {
                    self.bump();
                }
                let _ = allow_call;
                self.node(SyntaxKind::Error, start, self.position, Vec::new())
            }
        }
    }

    fn parse_type_expression(&mut self) -> SyntaxNode {
        let start = self.position;
        let mut left = self.parse_prefix_type();
        let mut members = vec![left];
        while self.at(TokenKind::Pipe) || self.at_text("|") {
            self.bump();
            members.push(self.parse_prefix_type());
        }
        left = if members.len() > 1 {
            self.node(SyntaxKind::UnionType, start, self.position, members)
        } else {
            members.remove(0)
        };
        self.node(SyntaxKind::TypeExpression, start, self.position, vec![left])
    }

    fn parse_prefix_type(&mut self) -> SyntaxNode {
        let start = self.position;
        if self.eat_text("ref") {
            let inner = self.parse_prefix_type();
            return self.node(SyntaxKind::PrefixType, start, self.position, vec![inner]);
        }
        if self.eat_text("function") {
            let mut children = Vec::new();
            if self.eat_text("from") {
                loop {
                    children.push(self.parse_type_expression());
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect_text("to", "S1020", "function type requires `to`");
            children.push(self.parse_type_expression());
            return self.node(SyntaxKind::FunctionType, start, self.position, children);
        }
        let mut base = if self.at(TokenKind::Identifier) {
            if self.text().contains('<') {
                self.error_here(
                    "S1092",
                    "angle-bracket generic syntax is unsupported; write `list of string`",
                );
            }
            self.leaf(SyntaxKind::Name)
        } else if self.eat(TokenKind::OpenParen) {
            let inner = self.parse_type_expression();
            self.expect(TokenKind::CloseParen, "S1021", "expected `)` after type");
            self.node(
                SyntaxKind::GroupExpression,
                start,
                self.position,
                vec![inner],
            )
        } else {
            self.error_here("S1022", "expected a type expression");
            self.node(SyntaxKind::Error, start, self.position, Vec::new())
        };
        if self.at_text("of") {
            self.bump();
            let mut args = vec![base];
            loop {
                args.push(self.parse_type_expression());
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            base = self.node(SyntaxKind::AppliedType, start, self.position, args);
        }
        if matches!(self.text(), ">" | ">=") {
            self.error_here(
                "S1092",
                "angle-bracket generic syntax is unsupported; write `list of string`",
            );
            self.bump();
        }
        base
    }

    fn parse_block(&mut self) -> SyntaxNode {
        let start = self.position;
        self.expect(
            TokenKind::Newline,
            "S1023",
            "expected a newline before block body",
        );
        self.skip_newlines();
        let mut children = Vec::new();
        if self.eat(TokenKind::Indent) {
            self.skip_newlines();
            while !self.at(TokenKind::Dedent) && !self.at(TokenKind::Eof) {
                children.push(self.parse_statement());
                self.finish_statement();
                self.skip_newlines();
            }
            self.expect(
                TokenKind::Dedent,
                "S1024",
                "expected the end of the indented block",
            );
        }
        self.node(SyntaxKind::Block, start, self.position, children)
    }

    fn finish_statement(&mut self) {
        if self.at(TokenKind::Newline) {
            self.bump();
        } else if self.position > 0
            && matches!(
                self.tokens[self.position - 1].kind,
                TokenKind::Newline | TokenKind::Dedent
            )
        {
            // Compound statements consume their body's final layout token.
        } else if !self.at(TokenKind::Dedent) && !self.at(TokenKind::Eof) {
            self.error_here("S1025", "unexpected content after statement");
            self.recover_line();
            if self.at(TokenKind::Newline) {
                self.bump();
            }
        }
    }

    fn binary_precedence(&self) -> Option<u8> {
        match self.text() {
            "or" => Some(1),
            "and" => Some(2),
            "is" => Some(3),
            "==" | "!=" | "<" | "<=" | ">" | ">=" => Some(4),
            "|" => Some(5),
            "^" => Some(6),
            "&" => Some(7),
            "<<" | ">>" => Some(8),
            "+" | "-" => Some(9),
            "*" | "/" | "%" => Some(10),
            _ => None,
        }
    }

    fn is_comparison(text: &str) -> bool {
        matches!(text, "==" | "!=" | "<" | "<=" | ">" | ">=")
    }
    fn require_expression(&mut self, context: &str) -> SyntaxNode {
        if self.at_expression_end() {
            self.error_here("S1019", format!("expected {context}"));
            self.node(SyntaxKind::Error, self.position, self.position, Vec::new())
        } else {
            self.parse_expression(0, true)
        }
    }

    fn looks_like_binding(&self) -> bool {
        if matches!(
            self.text(),
            "public" | "private" | "protected" | "global" | "constant"
        ) {
            return true;
        }
        self.at(TokenKind::Identifier)
            && self.peek_kind(1) == Some(TokenKind::Identifier)
            && !matches!(self.peek_text(1), Some("in" | "is" | "and" | "or"))
    }

    fn looks_like_function_declaration(&self) -> bool {
        self.tokens[self.position..]
            .iter()
            .take_while(|token| token.kind != TokenKind::Newline)
            .skip_while(|token| {
                matches!(
                    token.text.as_str(),
                    "public" | "private" | "protected" | "static" | "async" | "mutating" | "throws"
                )
            })
            .next()
            .is_some_and(|token| token.text == "function")
    }
    fn line_has_semicolons(&self, count: usize) -> bool {
        let mut depth = 0usize;
        let mut semicolons = 0usize;
        for token in self.tokens[self.position..]
            .iter()
            .take_while(|token| token.kind != TokenKind::Newline)
        {
            match token.kind {
                TokenKind::OpenParen | TokenKind::OpenBracket | TokenKind::OpenBrace => depth += 1,
                TokenKind::CloseParen | TokenKind::CloseBracket | TokenKind::CloseBrace => {
                    depth = depth.saturating_sub(1);
                }
                TokenKind::Semicolon if depth == 0 => semicolons += 1,
                _ => {}
            }
        }
        semicolons >= count
    }
    fn recover_line(&mut self) {
        while !self.at_line_end() {
            self.bump();
        }
    }
    fn recover_expression(&mut self) {
        while !self.at_expression_end() {
            self.bump();
        }
    }
    fn recover_to_comma_or_line(&mut self) {
        while !self.at(TokenKind::Comma) && !self.at_line_end() {
            self.bump();
        }
    }
    fn recover_nested_block(&mut self) {
        let mut depth = 1usize;
        while depth > 0 && !self.at(TokenKind::Eof) {
            match self.current().kind {
                TokenKind::Indent => depth += 1,
                TokenKind::Dedent => depth -= 1,
                _ => {}
            }
            self.bump();
        }
    }
    fn skip_newlines(&mut self) {
        while self.at(TokenKind::Newline) {
            self.bump();
        }
    }
    fn at_line_end(&self) -> bool {
        matches!(
            self.current().kind,
            TokenKind::Newline | TokenKind::Dedent | TokenKind::Eof
        )
    }
    fn at_expression_end(&self) -> bool {
        self.at_line_end()
            || matches!(
                self.current().kind,
                TokenKind::Comma | TokenKind::CloseParen | TokenKind::CloseBracket
            )
    }
    fn at(&self, kind: TokenKind) -> bool {
        self.current().kind == kind
    }
    fn at_text(&self, text: &str) -> bool {
        self.text() == text
    }
    fn text(&self) -> &str {
        &self.current().text
    }
    fn peek_kind(&self, offset: usize) -> Option<TokenKind> {
        self.tokens
            .get(self.position + offset)
            .map(|token| token.kind)
    }
    fn peek_text(&self, offset: usize) -> Option<&str> {
        self.tokens
            .get(self.position + offset)
            .map(|token| token.text.as_str())
    }
    fn type_starts_at(&self, offset: usize) -> bool {
        matches!(
            self.peek_kind(offset),
            Some(TokenKind::Identifier | TokenKind::OpenParen)
        )
    }
    fn current(&self) -> &Token {
        &self.tokens[self.position.min(self.tokens.len() - 1)]
    }
    fn bump(&mut self) {
        if !self.at(TokenKind::Eof) {
            self.position += 1;
        }
    }
    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn eat_text(&mut self, text: &str) -> bool {
        if self.at_text(text) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, kind: TokenKind, code: &'static str, message: &str) {
        if !self.eat(kind) {
            self.error_here(code, message);
        }
    }
    fn expect_text(&mut self, text: &str, code: &'static str, message: &str) {
        if !self.eat_text(text) {
            self.error_here(code, message);
        }
    }
    fn error_here(&mut self, code: &'static str, message: impl Into<String>) {
        self.diagnostics
            .push(Diagnostic::error(code, message, self.current().span));
    }
    fn error_at(&mut self, index: usize, code: &'static str, message: impl Into<String>) {
        let span = self
            .tokens
            .get(index)
            .map_or(self.current().span, |token| token.span);
        self.diagnostics
            .push(Diagnostic::error(code, message, span));
    }
    fn leaf(&mut self, kind: SyntaxKind) -> SyntaxNode {
        let start = self.position;
        self.bump();
        self.node(kind, start, self.position, Vec::new())
    }
    fn node(
        &self,
        kind: SyntaxKind,
        start: usize,
        end: usize,
        children: Vec<SyntaxNode>,
    ) -> SyntaxNode {
        let span = if start < end {
            Span::new(
                self.source.id(),
                self.tokens[start].span.start,
                self.tokens[end - 1].span.end,
            )
        } else {
            let offset = self.current().span.start;
            Span::new(self.source.id(), offset, offset)
        };
        SyntaxNode::new(kind, span, start..end, children)
    }
}
