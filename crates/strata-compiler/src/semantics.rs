use std::collections::BTreeMap;

use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxTree};
use crate::{Diagnostic, Package, SourceFile, Span, lexer, parser};

pub const BOOTSTRAP_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Public,
    Protected,
    Private,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Symbol {
    pub identity: String,
    pub name: String,
    pub namespace: String,
    pub object_form: bool,
    pub visibility: Visibility,
    pub global: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Namespace {
    pub ordinary: BTreeMap<String, Symbol>,
    pub objects: BTreeMap<String, Symbol>,
}

#[derive(Clone, Debug)]
pub struct SemanticPackage {
    pub identity: String,
    pub prelude: bool,
    pub namespaces: BTreeMap<String, Namespace>,
    pub units: Vec<SemanticUnit>,
    pub bootstrap_version: &'static str,
}

#[derive(Clone, Debug)]
pub struct SemanticFailure {
    pub source: SourceFile,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug)]
pub struct SemanticUnit {
    pub source: SourceFile,
    pub tree: SyntaxTree,
    pub namespace: String,
}

#[derive(Clone)]
struct Import {
    source: SourceFile,
    namespace: String,
    target: String,
    object: String,
    alias: String,
    span: Span,
}

/// Builds the complete namespace tree, then resolves declarations and imports.
///
/// # Errors
/// Returns source-oriented lexer, parser, namespace, scope, and import diagnostics.
pub fn analyze(package: &Package) -> Result<SemanticPackage, SemanticFailure> {
    let mut units = Vec::with_capacity(package.units.len());
    for unit in &package.units {
        let source = &unit.source;
        let lexed = lexer::lex(source).map_err(|diagnostics| SemanticFailure {
            source: source.clone(),
            diagnostics,
        })?;
        let parsed = parser::parse(source, lexed);
        if !parsed.diagnostics.is_empty() {
            return Err(SemanticFailure {
                source: source.clone(),
                diagnostics: parsed.diagnostics,
            });
        }
        let namespace =
            declared_namespace(source, &parsed.tree).map_err(|diagnostic| SemanticFailure {
                source: source.clone(),
                diagnostics: vec![diagnostic],
            })?;
        units.push(SemanticUnit {
            source: source.clone(),
            tree: parsed.tree,
            namespace,
        });
    }

    let mut namespaces = bootstrap_namespaces();
    for unit in &units {
        namespaces.entry(unit.namespace.clone()).or_default();
    }

    let mut imports = Vec::new();
    let mut globals = BTreeMap::<String, Symbol>::new();
    for unit in &units {
        collect_unit(unit, &mut namespaces, &mut globals, &mut imports)?;
    }
    resolve_imports(imports, &mut namespaces)?;
    if package.prelude {
        install_prelude(&mut namespaces);
    }

    Ok(SemanticPackage {
        identity: package.identity.clone(),
        prelude: package.prelude,
        namespaces,
        units,
        bootstrap_version: BOOTSTRAP_VERSION,
    })
}

impl SemanticPackage {
    #[must_use]
    pub fn ordinary(&self, namespace: &str, name: &str) -> Option<&Symbol> {
        self.namespaces.get(namespace)?.ordinary.get(name)
    }

    #[must_use]
    pub fn object(&self, namespace: &str, name: &str) -> Option<&Symbol> {
        self.namespaces.get(namespace)?.objects.get(name)
    }
}

fn declared_namespace(source: &SourceFile, tree: &SyntaxTree) -> Result<String, Diagnostic> {
    let declarations = tree
        .root
        .children
        .iter()
        .filter(|node| node.kind == SyntaxKind::NamespaceDeclaration)
        .collect::<Vec<_>>();
    if declarations.is_empty() {
        return Err(Diagnostic::error(
            "S2002",
            "each source unit must declare exactly one namespace",
            Span::new(source.id(), 0, source.text().len()),
        ));
    }
    if declarations.len() > 1 {
        return Err(Diagnostic::error(
            "S0005",
            "duplicate namespace declaration",
            declarations[1].span,
        ));
    }
    let text = node_text(source, declarations[0]);
    let path = text.trim().strip_prefix("namespace").unwrap().trim();
    normalize_declared_path(path).ok_or_else(|| {
        Diagnostic::error(
            "S2003",
            "namespace declaration requires an unanchored path",
            declarations[0].span,
        )
    })
}

fn collect_unit(
    unit: &SemanticUnit,
    namespaces: &mut BTreeMap<String, Namespace>,
    globals: &mut BTreeMap<String, Symbol>,
    imports: &mut Vec<Import>,
) -> Result<(), SemanticFailure> {
    for node in &unit.tree.root.children {
        match node.kind {
            SyntaxKind::Binding | SyntaxKind::Assignment | SyntaxKind::FunctionDeclaration => {
                collect_declaration(unit, node, namespaces, globals)?;
            }
            SyntaxKind::ImportDeclaration => imports.extend(parse_import(unit, node)?),
            _ => {}
        }
    }
    Ok(())
}

fn collect_declaration(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    namespaces: &mut BTreeMap<String, Namespace>,
    globals: &mut BTreeMap<String, Symbol>,
) -> Result<(), SemanticFailure> {
    let text = node_text(&unit.source, node).trim();
    let words = text.split_whitespace().collect::<Vec<_>>();
    let global = words.first() == Some(&"global");
    let visibility = if words.contains(&"private") {
        Visibility::Private
    } else if words.contains(&"protected") {
        Visibility::Protected
    } else {
        Visibility::Public
    };
    let name = declaration_name(node, &unit.source).ok_or_else(|| {
        failure(
            &unit.source,
            "S2004",
            "declaration has no resolvable name",
            node.span,
        )
    })?;
    let object_form = name.starts_with('.');
    let bare = name.trim_start_matches('.').to_owned();
    if bare == "import" && node.kind != SyntaxKind::ImportDeclaration {
        // Deliberately ordinary: import syntax was classified structurally by the parser.
    }
    let identity = if global {
        format!("global::{bare}")
    } else {
        format!("{}::{bare}", unit.namespace)
    };
    let symbol = Symbol {
        identity,
        name: bare.clone(),
        namespace: unit.namespace.clone(),
        object_form,
        visibility,
        global,
    };
    if global {
        globals.insert(bare, symbol);
        return Ok(());
    }
    let namespace = namespaces.get_mut(&unit.namespace).unwrap();
    let table = if object_form {
        &mut namespace.objects
    } else {
        &mut namespace.ordinary
    };
    if table.contains_key(&bare) {
        return Err(failure(
            &unit.source,
            "S2005",
            format!("duplicate declaration `{name}`"),
            node.span,
        ));
    }
    table.insert(bare, symbol);
    Ok(())
}

fn declaration_name(node: &SyntaxNode, source: &SourceFile) -> Option<String> {
    fn first_name<'a>(node: &'a SyntaxNode) -> Option<&'a SyntaxNode> {
        if matches!(node.kind, SyntaxKind::Name | SyntaxKind::ObjectName) {
            return Some(node);
        }
        node.children.iter().find_map(first_name)
    }
    let name = first_name(node)?;
    Some(node_text(source, name).trim().to_owned())
}

fn parse_import(unit: &SemanticUnit, node: &SyntaxNode) -> Result<Vec<Import>, SemanticFailure> {
    let text = node_text(&unit.source, node).trim();
    let (path, selection) = if let Some(rest) = text.strip_prefix("from ") {
        rest.split_once(" import ")
            .ok_or_else(|| failure(&unit.source, "S2006", "malformed import", node.span))?
    } else if let Some(rest) = text.strip_prefix("import ") {
        let (path, object) = rest
            .rsplit_once(' ')
            .ok_or_else(|| failure(&unit.source, "S2006", "malformed import", node.span))?;
        (path, object)
    } else {
        return Err(failure(
            &unit.source,
            "S2006",
            "malformed import",
            node.span,
        ));
    };
    let target = resolve_path(&unit.namespace, path).ok_or_else(|| {
        failure(
            &unit.source,
            "S2007",
            "namespace path escapes above root",
            node.span,
        )
    })?;
    let mut result = Vec::new();
    for item in selection.split(',') {
        let item = item.trim();
        let (object, alias) = item
            .split_once(" as ")
            .map_or((item, item), |(object, alias)| {
                (object.trim(), alias.trim())
            });
        if !object.starts_with('.') || !alias.starts_with('.') {
            return Err(failure(
                &unit.source,
                "S2008",
                "imports bind object-form names; use an explicit ordinary binding",
                node.span,
            ));
        }
        result.push(Import {
            source: unit.source.clone(),
            namespace: unit.namespace.clone(),
            target: target.clone(),
            object: object.trim_start_matches('.').to_owned(),
            alias: alias.trim_start_matches('.').to_owned(),
            span: node.span,
        });
    }
    Ok(result)
}

fn resolve_imports(
    imports: Vec<Import>,
    namespaces: &mut BTreeMap<String, Namespace>,
) -> Result<(), SemanticFailure> {
    for import in imports {
        let Some(export) = namespaces
            .get(&import.target)
            .and_then(|namespace| namespace.objects.get(&import.object))
            .cloned()
        else {
            return Err(failure(
                &import.source,
                "S2009",
                format!(
                    "unresolved object `.{}` in `{}`",
                    import.object, import.target
                ),
                import.span,
            ));
        };
        if export.visibility == Visibility::Private && export.namespace != import.namespace {
            return Err(failure(
                &import.source,
                "S2010",
                format!("object `.{}` is private", import.object),
                import.span,
            ));
        }
        let destination = namespaces.get_mut(&import.namespace).unwrap();
        if let Some(existing) = destination.objects.get(&import.alias) {
            if existing.identity == export.identity {
                continue;
            }
            return Err(failure(
                &import.source,
                "S2011",
                format!(
                    "object-form import `.{}` collides; use an alias",
                    import.alias
                ),
                import.span,
            ));
        }
        destination.objects.insert(import.alias, export);
    }
    Ok(())
}

fn install_prelude(namespaces: &mut BTreeMap<String, Namespace>) {
    const PRELUDE: [(&str, &str); 7] = [
        ("print", "/core/output::print"),
        ("int", "/core/types::int"),
        ("float", "/core/types::float"),
        ("bool", "/core/types::bool"),
        ("string", "/core/types::string"),
        ("bytes", "/core/types::bytes"),
        ("none", "/core/types::none"),
    ];
    for (path, namespace) in namespaces
        .iter_mut()
        .filter(|(path, _)| !path.starts_with("/core") && path.as_str() != "/collections")
    {
        for (name, identity) in PRELUDE {
            namespace
                .ordinary
                .entry(name.to_owned())
                .or_insert_with(|| Symbol {
                    identity: identity.to_owned(),
                    name: name.to_owned(),
                    namespace: path.clone(),
                    object_form: false,
                    visibility: Visibility::Public,
                    global: false,
                });
        }
    }
}

fn bootstrap_namespaces() -> BTreeMap<String, Namespace> {
    let mut namespaces = BTreeMap::new();
    namespaces.insert(
        "/core/output".to_owned(),
        namespace_with_objects("/core/output", ["print"]),
    );
    let mut types = vec![
        "int".to_owned(),
        "float".to_owned(),
        "bool".to_owned(),
        "string".to_owned(),
        "bytes".to_owned(),
        "none".to_owned(),
        "float32".to_owned(),
        "float64".to_owned(),
    ];
    for prefix in ["int", "uint"] {
        for width in [8, 16, 32, 64, 128] {
            types.push(format!("{prefix}{width}"));
        }
    }
    namespaces.insert(
        "/core/types".to_owned(),
        namespace_with_objects("/core/types", types.iter().map(String::as_str)),
    );
    namespaces.insert(
        "/core/errors".to_owned(),
        namespace_with_objects(
            "/core/errors",
            [
                "error",
                "arithmetic-overflow",
                "division-by-zero",
                "integer-conversion-overflow",
                "negative-shift-count",
                "coercion-error",
            ],
        ),
    );
    namespaces.insert("/collections".to_owned(), Namespace::default());
    namespaces
}

fn namespace_with_objects<'a>(path: &str, names: impl IntoIterator<Item = &'a str>) -> Namespace {
    let objects = names
        .into_iter()
        .map(|name| {
            (
                name.to_owned(),
                Symbol {
                    identity: format!("{path}::{name}"),
                    name: name.to_owned(),
                    namespace: path.to_owned(),
                    object_form: true,
                    visibility: Visibility::Public,
                    global: false,
                },
            )
        })
        .collect();
    Namespace {
        ordinary: BTreeMap::new(),
        objects,
    }
}

fn normalize_declared_path(path: &str) -> Option<String> {
    if path.starts_with('/') || path.starts_with("..") || path.is_empty() {
        return None;
    }
    Some(format!(
        "/{}",
        path.split_whitespace().collect::<Vec<_>>().join("/")
    ))
}

fn resolve_path(current: &str, path: &str) -> Option<String> {
    let mut components = if path.starts_with('/') {
        Vec::new()
    } else {
        current
            .trim_start_matches('/')
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
    };
    for component in path.trim_start_matches('/').split_whitespace() {
        if component == ".." {
            components.pop()?;
        } else {
            components.push(component);
        }
    }
    Some(format!("/{}", components.join("/")))
}

fn node_text<'a>(source: &'a SourceFile, node: &SyntaxNode) -> &'a str {
    &source.text()[node.span.start..node.span.end]
}

fn failure(
    source: &SourceFile,
    code: &'static str,
    message: impl Into<String>,
    span: Span,
) -> SemanticFailure {
    SemanticFailure {
        source: source.clone(),
        diagnostics: vec![Diagnostic::error(code, message, span)],
    }
}
