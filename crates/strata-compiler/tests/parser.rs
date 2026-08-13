use strata_compiler::{SourceFile, lexer::lex, parser::parse, syntax::SyntaxKind};

fn parse_source(text: &str) -> strata_compiler::syntax::SyntaxTree {
    let source = SourceFile::new(0, "case.strata".into(), text.to_owned());
    let lexed = lex(&source).unwrap();
    parse(&source, lexed)
        .unwrap_or_else(|diagnostics| panic!("unexpected parser diagnostics: {diagnostics:#?}"))
}

fn rejected(text: &str, code: &str) {
    let source = SourceFile::new(0, "case.strata".into(), text.to_owned());
    let lexed = lex(&source).unwrap();
    let diagnostics = parse(&source, lexed).unwrap_err();
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.code == code),
        "{diagnostics:#?}"
    );
}

fn contains(node: &strata_compiler::syntax::SyntaxNode, kind: SyntaxKind) -> bool {
    node.kind == kind || node.children.iter().any(|child| contains(child, kind))
}

#[test]
fn parses_lossless_declarations_and_legal_empty_blocks() {
    let text = "namespace example app\npublic count int = 1\nfunction empty; value int\nfunction main\n  count = count + 1\n";
    let tree = parse_source(text);
    assert!(contains(&tree.root, SyntaxKind::NamespaceDeclaration));
    assert!(contains(&tree.root, SyntaxKind::Binding));
    assert_eq!(
        tree.lexed.tokens.last().unwrap().kind,
        strata_compiler::tokens::TokenKind::Eof
    );
    assert!(tree.normalized().starts_with("CompilationUnit 0.."));
}

#[test]
fn expression_tree_respects_precedence_and_postfix_binding() {
    let tree = parse_source("result = -left + thing.member * values[1]\n");
    let assignment = &tree.root.children[0];
    assert_eq!(assignment.kind, SyntaxKind::Binding);
    let expression = assignment.children.last().unwrap();
    assert_eq!(expression.kind, SyntaxKind::BinaryExpression);
    assert!(contains(expression, SyntaxKind::UnaryExpression));
    assert!(contains(expression, SyntaxKind::MemberExpression));
    assert!(contains(expression, SyntaxKind::IndexExpression));
}

#[test]
fn calls_distinguish_object_lookup_zero_arguments_and_grouped_nesting() {
    let tree = parse_source("result = .thing;\nvalue = call; first, (convert; second)\n");
    assert!(contains(&tree.root, SyntaxKind::ObjectName));
    assert!(contains(&tree.root, SyntaxKind::CallExpression));
    assert!(contains(&tree.root, SyntaxKind::GroupExpression));
    rejected("value = call; convert; input\n", "S1016");
}

#[test]
fn rejects_spaced_member_access_and_chained_comparisons() {
    rejected("value = print .concat\n", "S1013");
    rejected("value = a < b < c\n", "S1012");
}

#[test]
fn tail_strings_remain_literals_while_comparisons_and_shifts_parse_as_operators() {
    let tree = parse_source("message = >text\nsmall = a > b\nshifted = a >> b\n");
    assert!(contains(&tree.root.children[0], SyntaxKind::Literal));
    assert_eq!(
        tree.root.children[1].children.last().unwrap().kind,
        SyntaxKind::BinaryExpression
    );
    assert_eq!(
        tree.root.children[2].children.last().unwrap().kind,
        SyntaxKind::BinaryExpression
    );
}
