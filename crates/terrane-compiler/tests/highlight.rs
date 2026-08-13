use terrane_compiler::SourceFile;
use terrane_compiler::highlight::{HighlightKind, highlight};

fn classified(source: &str) -> Vec<(String, HighlightKind, bool)> {
    let file = SourceFile::new(0, "case.trn".into(), source.to_owned());
    highlight(&file)
        .highlights
        .into_iter()
        .map(|item| {
            (
                source[item.span.start..item.span.end].to_owned(),
                item.kind,
                item.declaration,
            )
        })
        .collect()
}

#[test]
fn classifies_real_lexical_and_syntax_constructs() {
    let source = concat!(
        "namespace sample app\n",
        "from /core output import .print\n",
        "public function greet; name string\n",
        "  message = >>\n",
        "    Hello\n",
        "  print; message # output\n",
    );
    let actual = classified(source);

    for expected in [
        ("namespace", HighlightKind::Keyword, false),
        ("sample", HighlightKind::Namespace, false),
        ("app", HighlightKind::Namespace, false),
        ("core", HighlightKind::Namespace, false),
        ("output", HighlightKind::Namespace, false),
        ("print", HighlightKind::Property, false),
        ("greet", HighlightKind::Function, true),
        ("name", HighlightKind::Parameter, true),
        ("string", HighlightKind::Type, false),
        (">>\n    Hello", HighlightKind::String, false),
        ("print", HighlightKind::Function, false),
        ("# output", HighlightKind::Comment, false),
    ] {
        assert!(actual.contains(&(expected.0.to_owned(), expected.1, expected.2)), "missing {expected:?} in {actual:#?}");
    }
}

#[test]
fn retains_highlights_around_lexical_and_syntax_errors() {
    let source = "namespace app\ninvalid = @\nfunction main\n  value = 'ok'\n";
    let file = SourceFile::new(0, "broken.trn".into(), source.to_owned());
    let output = highlight(&file);

    assert!(output.diagnostics.iter().any(|diagnostic| diagnostic.code == "L0001"));
    assert!(output.highlights.iter().any(|item| {
        item.kind == HighlightKind::Function && item.declaration && &source[item.span.start..item.span.end] == "main"
    }));
    assert!(output.highlights.iter().any(|item| {
        item.kind == HighlightKind::String && &source[item.span.start..item.span.end] == "'ok'"
    }));
}
