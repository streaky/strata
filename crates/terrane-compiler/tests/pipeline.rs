use std::path::PathBuf;

const HELLO: &str = include_str!("../../../tests/conformance/run/hello/case.trn");

#[test]
fn hello_lowers_deterministically() {
    let first = terrane_compiler::compile(PathBuf::from("case.trn"), HELLO.to_owned()).unwrap();
    let second = terrane_compiler::compile(PathBuf::from("case.trn"), HELLO.to_owned()).unwrap();
    assert_eq!(first.rust, second.rust);
    assert!(
        first
            .rust
            .contains("Hello from Terrane!\\n\\nTail strings make punctuation literal")
    );
}

#[test]
fn inferred_local_first_assignment_lowers_as_a_declaration() {
    let source = "namespace inferred\nfunction main\n  total = 5\n  total = total + 1\n";
    let compilation = terrane_compiler::compile("inferred.trn", source.to_owned()).unwrap();

    assert!(
        compilation.rust.contains(
            "let total: terrane_int_support::Int = terrane_int_support::Int::from(5_i128);"
        )
    );
    assert!(compilation.rust.contains("let _ = &total;"));
    assert!(compilation.rust.contains(
        "let total: terrane_int_support::Int = total.clone() + terrane_int_support::Int::from(1_i128);"
    ));
}

#[test]
fn rejects_duplicate_declarations() {
    let cases = [
        (
            "namespace hello",
            "S0005",
            "duplicate namespace declaration",
        ),
        ("function main", "S2005", "duplicate declaration `main`"),
    ];

    for (construct, code, message) in cases {
        let source = HELLO.replacen(construct, &format!("{construct}\n{construct}"), 1);
        let diagnostics = terrane_compiler::compile("duplicate.trn", source).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code && diagnostic.message == message)
        );
    }
}

#[test]
fn rejects_mixed_indentation() {
    let source = HELLO.replace("  print", " \tprint");
    let diagnostics = terrane_compiler::compile("mixed.trn", source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "L0003")
    );
}

#[test]
fn blank_lines_do_not_select_indentation_style() {
    let source = HELLO
        .replace(
            "function main\n  print; >>",
            "function main\n \n\tprint; >>",
        )
        .replace("\n    Hello from Terrane!", "\n\t\tHello from Terrane!")
        .replace("\n    Tail strings", "\n\t\tTail strings");
    terrane_compiler::compile("blank-indent.trn", source).unwrap();
}

#[test]
fn permits_a_comment_after_a_closed_quote() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Terrane!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; 'hello' # trailing comment",
    );
    let compilation = terrane_compiler::compile("trailing-comment.trn", source).unwrap();
    assert!(compilation.rust.contains(
        "println!(\"{}\", terrane_scalar_support::scalar_text(&(String::from(\"hello\"))));"
    ));
}

#[test]
fn compilation_failure_owns_the_original_source() {
    let source = "namespace app\nfunction main\n  print; missing\n".to_owned();
    let failure = terrane_compiler::compile("owned.trn", source.clone()).unwrap_err();
    assert_eq!(failure.source.text(), source);
    assert_eq!(failure.source.path(), PathBuf::from("owned.trn").as_path());
    assert!(failure.iter().any(|diagnostic| diagnostic.code == "S2013"));
}

#[test]
fn tail_string_preserves_every_remaining_character() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Terrane!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; >Hello! From, \"Terrane\"! >> # literal",
    );
    let compilation = terrane_compiler::compile("tail.trn", source).unwrap();
    assert!(
        compilation
            .rust
            .contains("Hello! From, \\\"Terrane\\\"! >> # literal")
    );
}

#[test]
fn tail_string_can_be_empty() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Terrane!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; >",
    );
    let compilation = terrane_compiler::compile("empty-tail.trn", source).unwrap();
    assert!(
        compilation.rust.contains(
            "println!(\"{}\", terrane_scalar_support::scalar_text(&(String::from(\"\"))));"
        )
    );
}

#[test]
fn tail_string_preserves_leading_whitespace() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Terrane!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; > hello",
    );
    let compilation = terrane_compiler::compile("leading-space.trn", source).unwrap();
    assert!(compilation.rust.contains(
        "println!(\"{}\", terrane_scalar_support::scalar_text(&(String::from(\" hello\"))));"
    ));
}
#[test]
fn block_string_can_be_empty() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Terrane!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; >>",
    );
    let compilation = terrane_compiler::compile("string.trn", source).unwrap();
    assert!(
        compilation.rust.contains(
            "println!(\"{}\", terrane_scalar_support::scalar_text(&(String::from(\"\"))));"
        )
    );
}

#[test]
fn rejects_trailing_content_after_block_marker() {
    let source = HELLO.replace("print; >>", "print; >> ");
    let diagnostics = terrane_compiler::compile("marker.trn", source).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "L0008" && diagnostic.message.contains("final content")
    }));
}

#[test]
fn rejects_unresolved_name() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Terrane!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; missing",
    );
    let diagnostics = terrane_compiler::compile("name.trn", source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "S2013")
    );
}

#[test]
fn rejects_unresolved_call_argument() {
    let source = HELLO.replace(
        "print; >>\n    Hello from Terrane!\n\n    Tail strings make punctuation literal: >, #, \"quotes\".",
        "print; hello",
    );
    let diagnostics = terrane_compiler::compile("call.trn", source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "S2013")
    );
}

#[test]
fn compilation_uses_the_shared_parser_before_semantics() {
    let source = HELLO.replace("function main", "function main; ,");
    let diagnostics = terrane_compiler::compile("syntax.trn", source).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "S1007")
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "S0005")
    );
}

#[test]
fn lowers_collection_and_three_clause_for_loops_without_losing_continue_updates() {
    let collection = terrane_compiler::compile(
        "collection.trn",
        "namespace app\nfunction main\n  text string = 'ab'\n  for character in text\n    value = character\n"
            .to_owned(),
    )
    .unwrap();
    assert!(
        collection
            .rust
            .contains("for character in terrane_string_support::graphemes(&text) {")
    );

    let clauses = terrane_compiler::compile(
        "clauses.trn",
        "namespace app\nfunction main\n  for index = 0; index < 3; index++\n    if index == 1\n      continue\n"
            .to_owned(),
    )
    .unwrap();
    assert!(clauses.rust.contains("'__terrane_continue_0: {"));
    let continue_position = clauses.rust.find("break '__terrane_continue_0;").unwrap();
    let update_position = clauses
        .rust
        .find("index = index.clone() + terrane_int_support::Int::from(1_i128);")
        .unwrap();
    assert!(continue_position < update_position);
}

#[test]
fn lowers_scalar_membership_and_descriptor_identity_statically() {
    let source = concat!(
        "namespace descriptors\n",
        "from /core/types import int8 as byte, int8 as other-byte\n",
        "function accepts; item int\n",
        "  parameter-member = item is a int\n",
        "function main\n",
        "  value = 1\n",
        "  member = value is a int\n",
        "  same-descriptor = byte is byte\n",
        "  different-alias = byte is other-byte\n",
        "  same-scalar = value is value\n",
        "  same-value-type = value.type is value.type\n",
        "  different-value-type = value.type is byte.type\n",
    );
    let compilation = terrane_compiler::compile("descriptors.trn", source.to_owned()).unwrap();

    assert!(
        compilation
            .rust
            .contains("let member: bool = { let _ = value.clone(); true };")
    );
    assert!(
        compilation
            .rust
            .contains("let parameter_member: bool = { let _ = item.clone(); true };")
    );
    assert!(
        compilation
            .rust
            .contains("let same_descriptor: bool = {  true };")
    );
    assert!(
        compilation
            .rust
            .contains("let different_alias: bool = {  true };")
    );
    assert!(
        compilation
            .rust
            .contains("let same_scalar: bool = { let _ = value; let _ = value; false };")
    );
    assert!(
        compilation
            .rust
            .contains("let same_value_type: bool = { let _ = value; let _ = value; true };")
    );
    assert!(
        compilation
            .rust
            .contains("let different_value_type: bool = { let _ = value; false };")
    );
}

#[test]
fn lowers_named_arguments_into_parameter_order_with_defaults() {
    let source = concat!(
        "namespace calls\n",
        "function combine int; first int, second int = 2, third int = 3\n",
        "  return first + second + third\n",
        "function main\n",
        "  result = combine; 1, third = 9\n",
    );
    let compilation = terrane_compiler::compile("calls.trn", source.to_owned()).unwrap();

    assert!(compilation.rust.contains(
        "combine(terrane_int_support::Int::from(1_i128), \
terrane_int_support::Int::from(2_i128), \
terrane_int_support::Int::from(9_i128))"
    ));
}

#[test]
fn does_not_lower_shadowing_functions_as_builtins() {
    let source = concat!(
        "namespace shadowing\n",
        "function print int; value int\n",
        "  return value\n",
        "function main\n",
        "  result = print; 1\n",
    );
    let compilation = terrane_compiler::compile("shadowing.trn", source.to_owned()).unwrap();

    assert!(
        compilation
            .rust
            .contains("print(terrane_int_support::Int::from(1_i128))")
    );
    assert!(!compilation.rust.contains("println!"));
}

#[test]
fn unwraps_only_syntactic_condition_groups() {
    let source = concat!(
        "namespace conditions\n",
        "function main\n",
        "  if ((true))\n",
        "    print; 'yes'\n",
    );
    let compilation = terrane_compiler::compile("conditions.trn", source.to_owned()).unwrap();

    assert!(compilation.rust.contains("if true {"));
}

#[test]
fn lowers_logical_combinations_of_integer_comparisons() {
    let source = concat!(
        "namespace conditions\n",
        "function main\n",
        "  x int = 5\n",
        "  y int = 9\n",
        "  if x > 1 and y > 2\n",
        "    result = true\n",
    );
    let compilation = terrane_compiler::compile("conditions.trn", source.to_owned()).unwrap();

    assert!(compilation.rust.contains(
        "if (x.clone() > terrane_int_support::Int::from(1_i128)) && \
(y.clone() > terrane_int_support::Int::from(2_i128)) {"
    ));
}

#[test]
fn lowers_values_in_their_integer_destination_type() {
    let source = concat!(
        "namespace destinations\n",
        "function answer int\n",
        "  return 41\n",
        "function main\n",
        "  text = 'Terrane'\n",
        "  total int = text.length\n",
        "  total = total + 1\n",
    );
    let compilation = terrane_compiler::compile("destinations.trn", source.to_owned()).unwrap();

    assert!(
        compilation
            .rust
            .contains("return terrane_int_support::Int::from(41_i128);")
    );
    assert!(compilation.rust.contains(
        "let total: terrane_int_support::Int = terrane_int_support::Int::from(\
terrane_string_support::length(&text) as i128);"
    ));
    assert!(compilation.rust.contains("let _ = &total;"));
    assert!(compilation.rust.contains(
        "let total: terrane_int_support::Int = total.clone() + terrane_int_support::Int::from(1_i128);"
    ));
}

#[test]
fn lowers_fixed_width_arithmetic_through_checked_runtime_operations() {
    let source = concat!(
        "namespace fixed\n",
        "from /core/types import int8\n",
        "function main\n",
        "  left int8 = 120\n",
        "  right int8 = 10\n",
        "  sum int8 = left + right\n",
        "  quotient int8 = left / right\n",
        "  shifted int8 = left << right\n",
    );
    let compilation = terrane_compiler::compile("fixed.trn", source.to_owned()).unwrap();

    assert!(compilation.rust.contains(
        "terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_addition(left, right))"
    ));
    assert!(compilation.rust.contains(
        "terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_division(left, right))"
    ));
    assert!(compilation.rust.contains(
        "terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_shift_left(left, &right))"
    ));
}
