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

#[test]
fn parses_control_flow_and_recovers_at_layout_boundaries() {
    let tree = parse_source(
        "function main\n  if ready\n    return value\n  else\n  while running\n    continue\n  for item in values\n    break\n  for i = 0; i < 3; i++\n    value = i\n",
    );
    assert!(contains(&tree.root, SyntaxKind::IfStatement));
    assert!(contains(&tree.root, SyntaxKind::ElseClause));
    assert!(contains(&tree.root, SyntaxKind::WhileStatement));
    assert!(contains(&tree.root, SyntaxKind::ForStatement));
    assert!(contains(&tree.root, SyntaxKind::ReturnStatement));
    assert!(contains(&tree.root, SyntaxKind::BreakStatement));
    assert!(contains(&tree.root, SyntaxKind::ContinueStatement));
}

#[test]
fn three_clause_for_requires_grouping_for_calls() {
    parse_source("for i = (next;); i < limit; i++\n");
    rejected("for i = next; value; i < limit; i++\n", "S1016");
}

#[test]
fn preserves_type_shapes_without_keywording_core_names() {
    let tree = parse_source(
        "value list of string\nmaybe int | none\ncallback function from int, string to bool\nborrowed ref bytes\n",
    );
    assert!(contains(&tree.root, SyntaxKind::AppliedType));
    assert!(contains(&tree.root, SyntaxKind::UnionType));
    assert!(contains(&tree.root, SyntaxKind::FunctionType));
    assert!(contains(&tree.root, SyntaxKind::PrefixType));
}

#[test]
fn distinguishes_identity_from_type_membership() {
    let tree = parse_source("same = value is a\nmember = value is a int\n");
    assert_eq!(
        tree.root.children[0].children.last().unwrap().kind,
        SyntaxKind::BinaryExpression
    );
    assert_eq!(
        tree.root.children[1].children.last().unwrap().kind,
        SyntaxKind::TypeMembershipExpression
    );
}

#[test]
fn deferred_spellings_receive_canonical_fixes() {
    rejected("same = left === right\n", "S1091");
    rejected("items list<string>\n", "S1092");
    rejected("items list<string>= value\n", "S1092");
}

#[test]
fn normalized_tree_retains_tokens_and_trivia() {
    assert_eq!(
        parse_source("value = 1 # note\n").normalized(),
        concat!(
            "CompilationUnit 0..17\n",
            "  Binding 0..9\n",
            "    Name 0..5\n",
            "    Literal 8..9\n",
            "tokens\n",
            "  Identifier 0..5 \"value\"\n",
            "  Assign 6..7 \"=\"\n",
            "  Number 8..9 \"1\"\n",
            "  Newline 16..17 \"\\n\"\n",
            "  Eof 17..17 \"\"\n",
            "trivia\n",
            "  Whitespace 5..6 \" \"\n",
            "  Whitespace 7..8 \" \"\n",
            "  Whitespace 9..10 \" \"\n",
            "  LineComment 10..16 \"# note\"\n",
        )
    );
}

#[test]
fn parses_structural_import_forms_and_named_arguments() {
    let tree = parse_source(
        "from /core output import .print, .debug as .trace\nimport with .sandboxed-import\nvalue = render; input, width = 80\n",
    );
    assert!(contains(&tree.root, SyntaxKind::ImportDeclaration));
    assert!(contains(&tree.root, SyntaxKind::ImportSelection));
    assert!(contains(&tree.root, SyntaxKind::CallExpression));
    assert_eq!(
        tree.root.children[2].children.last().unwrap().children[1]
            .children
            .len(),
        2
    );
}

#[test]
fn rejects_malformed_declarations_and_reserved_constructs() {
    rejected("namespace\n", "S1002");
    rejected("value =\n", "S1004");
    rejected("function main; ,\n", "S1007");
    rejected("from import .thing\n", "S1026");
    rejected("import .thing\n", "S1027");
    rejected("class thing\n", "S1090");
}

#[test]
fn rejects_invalid_postfix_and_control_flow_boundaries() {
    rejected("value = thing.\n", "S1014");
    rejected("value = values[\n", "S1019");
    rejected("break value\n", "S1011");
    rejected("for item values\n", "S1009");
    rejected("if\n", "S1019");
}
