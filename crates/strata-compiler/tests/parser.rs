use strata_compiler::{SourceFile, lexer::lex, parser::parse, syntax::SyntaxKind};

fn parse_source(text: &str) -> strata_compiler::syntax::SyntaxTree {
    let source = SourceFile::new(0, "case.strata".into(), text.to_owned());
    let lexed = lex(&source).unwrap();
    let parsed = parse(&source, lexed);
    assert!(
        parsed.diagnostics.is_empty(),
        "unexpected parser diagnostics: {:#?}",
        parsed.diagnostics
    );
    parsed.tree
}

fn rejected(text: &str, code: &str) {
    let source = SourceFile::new(0, "case.strata".into(), text.to_owned());
    let lexed = lex(&source).unwrap();
    let diagnostics = parse(&source, lexed).diagnostics;
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.code == code),
        "{diagnostics:#?}"
    );
}

#[test]
fn returns_recovered_tree_and_tokens_with_diagnostics() {
    let source = SourceFile::new(0, "case.strata".into(), "value =\nnext = 1\n".to_owned());
    let lexed = lex(&source).unwrap();
    let parsed = parse(&source, lexed);
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "S1019")
    );
    assert_eq!(parsed.tree.root.children.len(), 2);
    assert_eq!(
        parsed.tree.lexed.tokens.last().unwrap().kind,
        strata_compiler::tokens::TokenKind::Eof
    );
}

fn contains(node: &strata_compiler::syntax::SyntaxNode, kind: SyntaxKind) -> bool {
    node.kind == kind || node.children.iter().any(|child| contains(child, kind))
}

#[test]
fn parses_lossless_declarations_and_legal_empty_blocks() {
    let text = "namespace example app\n.cache public constant count int = 1\n.trace public async throws function empty; value int\nfunction main\n  count = count + 1\n";
    let tree = parse_source(text);
    assert!(contains(&tree.root, SyntaxKind::NamespaceDeclaration));
    assert!(contains(&tree.root, SyntaxKind::Binding));
    assert!(contains(&tree.root, SyntaxKind::DeclarationModifier));
    assert!(contains(&tree.root, SyntaxKind::Visibility));
    assert!(contains(&tree.root, SyntaxKind::DeclarationQualifier));
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
    assert_eq!(assignment.kind, SyntaxKind::Assignment);
    let expression = assignment.children.last().unwrap();
    assert_eq!(expression.kind, SyntaxKind::BinaryExpression);
    assert!(contains(expression, SyntaxKind::UnaryExpression));
    assert!(contains(expression, SyntaxKind::MemberExpression));
    assert!(contains(expression, SyntaxKind::IndexExpression));
}

#[test]
fn parses_both_postfix_increment_and_decrement() {
    let tree = parse_source("left++\nright--\n");
    assert_eq!(tree.root.children.len(), 2);
    for expression in &tree.root.children {
        assert_eq!(expression.kind, SyntaxKind::PostfixExpression);
    }
}

#[test]
fn bare_names_are_expression_statements() {
    let tree = parse_source("thing\nfunction main\n  thing\n");
    assert_eq!(tree.root.children[0].kind, SyntaxKind::Name);
    assert_eq!(
        tree.root.children[1].children.last().unwrap().children[0].kind,
        SyntaxKind::Name
    );
}

#[test]
fn calls_distinguish_object_lookup_zero_arguments_and_grouped_nesting() {
    let tree = parse_source(
        ".print; 'hello'\n.thing\nresult = .thing;\nvalue = call; first, (convert; second)\n",
    );
    assert_eq!(tree.root.children[0].kind, SyntaxKind::CallExpression);
    assert_eq!(tree.root.children[1].kind, SyntaxKind::ObjectName);
    assert!(contains(&tree.root, SyntaxKind::ObjectName));
    assert!(contains(&tree.root, SyntaxKind::CallExpression));
    assert!(contains(&tree.root, SyntaxKind::GroupExpression));
    let source = SourceFile::new(
        0,
        "case.strata".into(),
        "value = call; convert; input\n".to_owned(),
    );
    let diagnostic = parse(&source, lex(&source).unwrap())
        .diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.code == "S1016")
        .unwrap();
    assert_eq!(
        diagnostic.help.as_deref(),
        Some("parenthesize the nested call, for example `outer; (inner; value)`")
    );
    assert!(
        diagnostic
            .render(&source)
            .contains("\n  help: parenthesize")
    );
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
        "function main\n  if ready\n    return value\n  else\n  while running\n    continue\n  for key, value in values\n    break\n  for i = 0; i < 3; i++\n    value = i\n",
    );
    assert!(contains(&tree.root, SyntaxKind::IfStatement));
    assert!(contains(&tree.root, SyntaxKind::ElseClause));
    assert!(contains(&tree.root, SyntaxKind::WhileStatement));
    assert!(contains(&tree.root, SyntaxKind::ForStatement));
    assert!(contains(&tree.root, SyntaxKind::ReturnStatement));
    assert!(contains(&tree.root, SyntaxKind::BreakStatement));
    assert!(contains(&tree.root, SyntaxKind::ContinueStatement));
    assert!(contains(&tree.root, SyntaxKind::ForTarget));
}

#[test]
fn three_clause_for_requires_grouping_for_calls() {
    parse_source("for i = (next;); i < limit; i++\n");
    rejected("for i = next; value; i < limit; i++\n", "S1016");
    parse_source("for item in (values; a, (b; c))\n  break\n");
    for text in [
        "for item.member in values\n  break\n",
        "for item + other in values\n  break\n",
    ] {
        let source = SourceFile::new(0, "case.strata".into(), text.to_owned());
        let parsed = parse(&source, lex(&source).unwrap());
        assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
        assert_eq!(parsed.diagnostics[0].code, "S1009");
        assert!(contains(&parsed.tree.root, SyntaxKind::Block));
        assert!(contains(&parsed.tree.root, SyntaxKind::BreakStatement));
    }
}

#[test]
fn preserves_type_shapes_without_keywording_core_names() {
    let tree = parse_source(
        "value list of string\nmaybe int | none\ncallback function from int, string to bool\npublic visible-callback function from int to bool\nconstant stable-callback function to bool\nborrowed ref bytes\n",
    );
    assert!(contains(&tree.root, SyntaxKind::AppliedType));
    assert!(contains(&tree.root, SyntaxKind::UnionType));
    assert!(contains(&tree.root, SyntaxKind::FunctionType));
    assert!(contains(&tree.root, SyntaxKind::PrefixType));
    assert_eq!(tree.root.children[3].kind, SyntaxKind::Binding);
    assert!(contains(&tree.root.children[3], SyntaxKind::FunctionType));
}

#[test]
fn distinguishes_identity_from_type_membership() {
    let tree = parse_source(
        "same = value is a\nmember = value is a int\ncomputed = count + 1 is a int\nready = value is a int and enabled\ngrouped = value is a (int | none)\n",
    );
    assert_eq!(
        tree.root.children[0].children.last().unwrap().kind,
        SyntaxKind::BinaryExpression
    );
    assert_eq!(
        tree.root.children[1].children.last().unwrap().kind,
        SyntaxKind::TypeMembershipExpression
    );
    assert_eq!(
        tree.root.children[2].children.last().unwrap().kind,
        SyntaxKind::TypeMembershipExpression
    );
    assert_eq!(
        tree.root.children[3].children.last().unwrap().kind,
        SyntaxKind::BinaryExpression
    );
    assert!(contains(
        tree.root.children[4].children.last().unwrap(),
        SyntaxKind::GroupExpression
    ));
}

#[test]
fn rejects_tighter_operators_after_type_membership() {
    for text in [
        "value = thing is a int + 1\n",
        "value = thing is a int == other\n",
    ] {
        let source = SourceFile::new(0, "case.strata".into(), text.to_owned());
        let diagnostics = parse(&source, lex(&source).unwrap()).diagnostics;
        assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
        assert_eq!(diagnostics[0].code, "S1012");
    }
}

#[test]
fn deferred_spellings_receive_canonical_fixes() {
    let source = SourceFile::new(
        0,
        "case.strata".into(),
        "same = left === right\n".to_owned(),
    );
    let diagnostics = parse(&source, lex(&source).unwrap()).diagnostics;
    assert_eq!(
        diagnostics[0].help.as_deref(),
        Some("use `==` for equality or `is` for identity")
    );
    for text in ["items list<string>\n", "items list<string>= value\n"] {
        let source = SourceFile::new(0, "case.strata".into(), text.to_owned());
        let lexed = lex(&source).unwrap();
        let diagnostics = parse(&source, lexed).diagnostics;
        assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
        assert_eq!(diagnostics[0].code, "S1092");
        assert_eq!(
            diagnostics[0].help.as_deref(),
            Some("write `list of string`")
        );
    }
}

#[test]
fn binary_word_operators_do_not_look_like_bindings() {
    for operator in ["in", "is", "and", "or"] {
        let text = format!("left {operator} right\n");
        let source = SourceFile::new(0, "case.strata".into(), text);
        let parsed = parse(&source, lex(&source).unwrap());
        assert_ne!(parsed.tree.root.children[0].kind, SyntaxKind::Binding);
        if let Some(diagnostic) = parsed.diagnostics.first() {
            assert_eq!(diagnostic.primary.unwrap().start, 5);
        }
    }
}

#[test]
fn normalized_tree_retains_tokens_and_trivia() {
    assert_eq!(
        parse_source("value = 1 # note\n").normalized(),
        concat!(
            "CompilationUnit 0..17\n",
            "  Assignment 0..9\n",
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
fn normalized_import_tree_retains_anchors_and_aliases() {
    assert_eq!(
        parse_source("from /core output import .debug as .trace\n").normalized(),
        concat!(
            "CompilationUnit 0..42\n",
            "  ImportDeclaration 0..41\n",
            "    NamespacePath 5..17\n",
            "      NamespaceAnchor 5..6\n",
            "      Name 6..10\n",
            "      Name 11..17\n",
            "    ObjectImport 25..41\n",
            "      ObjectName 25..31\n",
            "        Name 26..31\n",
            "      ImportAlias 32..41\n",
            "        ObjectName 35..41\n",
            "          Name 36..41\n",
            "tokens\n",
            "  Identifier 0..4 \"from\"\n",
            "  Operator 5..6 \"/\"\n",
            "  Identifier 6..10 \"core\"\n",
            "  Identifier 11..17 \"output\"\n",
            "  Identifier 18..24 \"import\"\n",
            "  Dot 25..26 \".\"\n",
            "  Identifier 26..31 \"debug\"\n",
            "  Identifier 32..34 \"as\"\n",
            "  Dot 35..36 \".\"\n",
            "  Identifier 36..41 \"trace\"\n",
            "  Newline 41..42 \"\\n\"\n",
            "  Eof 42..42 \"\"\n",
            "trivia\n",
            "  Whitespace 4..5 \" \"\n",
            "  Whitespace 10..11 \" \"\n",
            "  Whitespace 17..18 \" \"\n",
            "  Whitespace 24..25 \" \"\n",
            "  Whitespace 31..32 \" \"\n",
            "  Whitespace 34..35 \" \"\n",
        )
    );
}

#[test]
fn parses_structural_import_forms_and_named_arguments() {
    let tree = parse_source(
        "from /core output import .print, .debug as .trace\nfrom .. sibling import .item\nimport with .sandboxed-import\nvalue = render; input, width = 80\n",
    );
    let import = &tree.root.children[0];
    assert_eq!(import.kind, SyntaxKind::ImportDeclaration);
    assert_eq!(import.children[0].kind, SyntaxKind::NamespacePath);
    assert_eq!(import.children[0].children.len(), 3);
    assert_eq!(
        import.children[0].children[0].kind,
        SyntaxKind::NamespaceAnchor
    );
    assert_eq!(
        tree.root.children[1].children[0].children[0].kind,
        SyntaxKind::NamespaceAnchor
    );
    assert_eq!(import.children[1].kind, SyntaxKind::ObjectImport);
    assert_eq!(import.children[2].kind, SyntaxKind::ObjectImport);
    assert_eq!(import.children[2].children[1].kind, SyntaxKind::ImportAlias);
    assert_eq!(
        tree.root.children[2].children[0].kind,
        SyntaxKind::ObjectName
    );
    assert!(contains(&tree.root, SyntaxKind::CallExpression));
    assert_eq!(
        tree.root.children[3].children.last().unwrap().children[1]
            .children
            .len(),
        2
    );
}

#[test]
fn rejects_malformed_declarations_and_reserved_constructs() {
    rejected("namespace\n", "S1002");
    rejected("value =\n", "S1019");
    rejected("function main; ,\n", "S1007");
    let source = SourceFile::new(
        0,
        "case.strata".into(),
        "public private value int\n".to_owned(),
    );
    let diagnostics = parse(&source, lex(&source).unwrap()).diagnostics;
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(diagnostics[0].code, "S1029");
    rejected("constant global value int\n", "S1029");
    for text in ["constant public value int\n", "global private value int\n"] {
        let source = SourceFile::new(0, "case.strata".into(), text.to_owned());
        let diagnostics = parse(&source, lex(&source).unwrap()).diagnostics;
        assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
        assert_eq!(diagnostics[0].code, "S1029");
    }
    for text in ["global function main\n", "constant function main\n"] {
        let source = SourceFile::new(0, "case.strata".into(), text.to_owned());
        let diagnostics = parse(&source, lex(&source).unwrap()).diagnostics;
        assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
        assert_eq!(diagnostics[0].code, "S1029");
    }
    rejected("async async function work\n", "S1029");
    rejected("function map of T; value T\n", "S1090");
    rejected("function main; values int ...\n", "S1090");
    rejected("value = await thing\n", "S1090");
    rejected("value = move thing\n", "S1090");
    rejected("value = ref thing\n", "S1090");
    rejected("catch problem\n", "S1090");
    rejected("finally\n", "S1090");
    rejected("case value\n", "S1090");
    rejected("from import .thing\n", "S1026");
    rejected("import .thing\n", "S1027");
    rejected("from /core output import print\n", "S1026");
    rejected("from /core output import .print, , ]\n", "S1026");
    rejected("import with anything at all\n", "S1027");
    rejected("class thing\n", "S1090");
}

#[test]
fn rejects_every_reserved_statement_keyword() {
    for keyword in [
        "class", "try", "throw", "yield", "match", "unsafe", "rust", "label", "goto", "when",
        "use", "catch", "finally", "case",
    ] {
        rejected(&format!("{keyword}\n"), "S1090");
    }
}

#[test]
fn rejects_invalid_postfix_and_control_flow_boundaries() {
    rejected("value = thing.\n", "S1014");
    rejected("value = values[\n", "S1019");
    rejected("break value\n", "S1011");
    rejected("for item values\n", "S1009");
    rejected("if\n", "S1019");
}

#[test]
fn rejects_non_associative_identity_and_recovers_layout_errors_once() {
    rejected("same = a is b is c\n", "S1012");
    rejected("same = a is a int is b\n", "S1012");

    for (text, code) in [
        ("value = 1\n    deeper = 2\n", "S1001"),
        ("if a = b\n    value = 1\n", "S1030"),
        ("while a = b\n    value = 1\n", "S1030"),
    ] {
        let source = SourceFile::new(0, "case.strata".into(), text.to_owned());
        let diagnostics = parse(&source, lex(&source).unwrap()).diagnostics;
        assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
        assert_eq!(diagnostics[0].code, code);
    }

    let source = SourceFile::new(
        0,
        "case.strata".into(),
        "if a = b\n    value = 1\n".to_owned(),
    );
    let diagnostics = parse(&source, lex(&source).unwrap()).diagnostics;
    assert_eq!(
        diagnostics[0].help.as_deref(),
        Some("use `==` for equality")
    );

    let source = SourceFile::new(
        0,
        "case.strata".into(),
        "value = 1\n    deeper = 2\nafter = 3\n".to_owned(),
    );
    let parsed = parse(&source, lex(&source).unwrap());
    assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
    assert_eq!(parsed.tree.root.children.len(), 2);

    let source = SourceFile::new(
        0,
        "case.strata".into(),
        "function main\n  value = 1\n    deeper = 2\n  after = 3\n".to_owned(),
    );
    let parsed = parse(&source, lex(&source).unwrap());
    assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
    assert_eq!(parsed.diagnostics[0].code, "S1001");
    let block = parsed.tree.root.children[0].children.last().unwrap();
    assert_eq!(block.children.len(), 2);
}
