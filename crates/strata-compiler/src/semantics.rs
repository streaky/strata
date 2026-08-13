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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    Binding,
    Function,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Symbol {
    pub identity: String,
    pub name: String,
    pub namespace: String,
    pub object_form: bool,
    pub visibility: Visibility,
    pub global: bool,
    pub kind: SymbolKind,
    pub declaration_span: Option<Span>,
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
    pub globals: BTreeMap<String, Symbol>,
    pub prelude_bindings: BTreeMap<String, Symbol>,
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
    pub scopes: Vec<LexicalScope>,
}

#[derive(Clone, Debug)]
pub struct LexicalScope {
    pub span: Span,
    pub parent: Option<usize>,
    pub ordinary: BTreeMap<String, Symbol>,
    pub objects: BTreeMap<String, Symbol>,
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
            scopes: Vec::new(),
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
    for unit in &mut units {
        unit.scopes = collect_lexical_scopes(unit, &namespaces)?;
    }
    let prelude_bindings = if package.prelude {
        bootstrap_prelude()
    } else {
        BTreeMap::new()
    };

    let semantic = SemanticPackage {
        identity: package.identity.clone(),
        prelude: package.prelude,
        namespaces,
        globals,
        prelude_bindings,
        units,
        bootstrap_version: BOOTSTRAP_VERSION,
    };
    validate_references(&semantic)?;
    Ok(semantic)
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

    #[must_use]
    pub fn resolve_ordinary(&self, namespace: &str, name: &str) -> Option<&Symbol> {
        namespace_chain(namespace)
            .find_map(|path| {
                self.ordinary(&path, name)
                    .filter(|symbol| visible_from(symbol, namespace))
            })
            .or_else(|| {
                self.globals
                    .get(name)
                    .filter(|symbol| visible_from(symbol, namespace))
            })
            .or_else(|| self.prelude_bindings.get(name))
    }

    #[must_use]
    pub fn resolve_object(&self, namespace: &str, name: &str) -> Option<&Symbol> {
        namespace_chain(namespace).find_map(|path| {
            self.object(&path, name)
                .filter(|symbol| visible_from(symbol, namespace))
        })
    }

    #[must_use]
    pub fn resolve_ordinary_at<'a>(
        &'a self,
        unit: &'a SemanticUnit,
        offset: usize,
        name: &str,
    ) -> Option<&'a Symbol> {
        lexical_scope_chain(unit, offset)
            .find_map(|scope| scope.ordinary.get(name))
            .or_else(|| self.resolve_ordinary(&unit.namespace, name))
    }

    #[must_use]
    pub fn resolve_object_at<'a>(
        &'a self,
        unit: &'a SemanticUnit,
        offset: usize,
        name: &str,
    ) -> Option<&'a Symbol> {
        lexical_scope_chain(unit, offset)
            .find_map(|scope| scope.objects.get(name))
            .or_else(|| self.resolve_object(&unit.namespace, name))
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
    let components = declarations[0]
        .children
        .iter()
        .filter(|child| child.kind == SyntaxKind::Name)
        .map(|child| node_text(source, child))
        .collect::<Vec<_>>();
    normalize_declared_path(&components).ok_or_else(|| {
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
            SyntaxKind::ImportDeclaration => imports.extend(imports_from_syntax(unit, node)?),
            _ => {}
        }
        collect_nested_declarations(unit, node, namespaces, globals)?;
    }
    Ok(())
}
fn collect_nested_declarations(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    namespaces: &mut BTreeMap<String, Namespace>,
    globals: &mut BTreeMap<String, Symbol>,
) -> Result<(), SemanticFailure> {
    for child in &node.children {
        if matches!(
            child.kind,
            SyntaxKind::Binding | SyntaxKind::Assignment | SyntaxKind::FunctionDeclaration
        ) {
            if let Some(declaration) = declaration_from_syntax(unit, child) {
                if declaration.object_form {
                    return Err(failure(
                        &unit.source,
                        "S2017",
                        "object-form declarations inside lexical scopes are unsupported",
                        child.span,
                    ));
                }
                if declaration.global {
                    collect_declaration(unit, child, namespaces, globals)?;
                }
            }
        }
        collect_nested_declarations(unit, child, namespaces, globals)?;
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct Declaration {
    name: String,
    object_form: bool,
    visibility: Visibility,
    global: bool,
    kind: SymbolKind,
}

fn declaration_from_syntax(unit: &SemanticUnit, node: &SyntaxNode) -> Option<Declaration> {
    let name_node = node
        .children
        .iter()
        .find(|child| matches!(child.kind, SyntaxKind::Name | SyntaxKind::ObjectName))?;
    let object_form = name_node.kind == SyntaxKind::ObjectName;
    let name = node_text(&unit.source, name_node)
        .trim_start_matches('.')
        .to_owned();
    let visibility = node
        .children
        .iter()
        .find(|child| child.kind == SyntaxKind::Visibility)
        .map(|child| node_text(&unit.source, child))
        .map_or(Visibility::Public, |visibility| match visibility {
            "private" => Visibility::Private,
            "protected" => Visibility::Protected,
            _ => Visibility::Public,
        });
    let global = node.children.iter().any(|child| {
        child.kind == SyntaxKind::DeclarationQualifier && node_text(&unit.source, child) == "global"
    });
    let kind = if node.kind == SyntaxKind::FunctionDeclaration {
        SymbolKind::Function
    } else {
        SymbolKind::Binding
    };
    Some(Declaration {
        name,
        object_form,
        visibility,
        global,
        kind,
    })
}

fn collect_declaration(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    namespaces: &mut BTreeMap<String, Namespace>,
    globals: &mut BTreeMap<String, Symbol>,
) -> Result<(), SemanticFailure> {
    let declaration = declaration_from_syntax(unit, node).ok_or_else(|| {
        failure(
            &unit.source,
            "S2004",
            "declaration has no resolvable name",
            node.span,
        )
    })?;
    let identity = if declaration.global {
        format!("global::{}", declaration.name)
    } else {
        format!("{}::{}", unit.namespace, declaration.name)
    };
    let symbol = Symbol {
        identity,
        name: declaration.name.clone(),
        namespace: unit.namespace.clone(),
        object_form: declaration.object_form,
        visibility: declaration.visibility,
        global: declaration.global,
        kind: declaration.kind,
        declaration_span: Some(node.span),
    };
    if declaration.global {
        globals.insert(declaration.name, symbol);
        return Ok(());
    }
    let namespace = namespaces
        .get_mut(&unit.namespace)
        .expect("every source-unit namespace is assembled before declarations");
    let table = if declaration.object_form {
        &mut namespace.objects
    } else {
        &mut namespace.ordinary
    };
    if table.contains_key(&declaration.name) {
        return Err(failure(
            &unit.source,
            "S2005",
            format!("duplicate declaration `{}`", declaration.name),
            node.span,
        ));
    }
    table.insert(declaration.name, symbol);
    Ok(())
}

fn imports_from_syntax(
    unit: &SemanticUnit,
    node: &SyntaxNode,
) -> Result<Vec<Import>, SemanticFailure> {
    let path = node
        .children
        .iter()
        .find(|child| child.kind == SyntaxKind::NamespacePath)
        .ok_or_else(|| failure(&unit.source, "S2006", "malformed import", node.span))?;
    let anchored = path.children.first().is_some_and(|child| {
        child.kind == SyntaxKind::NamespaceAnchor && node_text(&unit.source, child) == "/"
    });
    let components = path
        .children
        .iter()
        .map(|child| node_text(&unit.source, child))
        .collect::<Vec<_>>();
    let target = resolve_path(&unit.namespace, anchored, &components).ok_or_else(|| {
        failure(
            &unit.source,
            "S2007",
            "namespace path escapes above root",
            path.span,
        )
    })?;
    let objects = node
        .children
        .iter()
        .filter(|child| child.kind == SyntaxKind::ObjectImport);
    let mut result = Vec::new();
    for object in objects {
        let imported_node = object
            .children
            .iter()
            .find(|child| child.kind == SyntaxKind::ObjectName)
            .ok_or_else(|| {
                failure(
                    &unit.source,
                    "S2008",
                    "imports bind object-form names; use an explicit ordinary binding",
                    object.span,
                )
            })?;
        let imported = node_text(&unit.source, imported_node).trim_start_matches('.');
        let alias = object
            .children
            .iter()
            .find(|child| child.kind == SyntaxKind::ImportAlias)
            .and_then(|alias| {
                alias
                    .children
                    .iter()
                    .find(|child| child.kind == SyntaxKind::ObjectName)
            })
            .map(|alias| node_text(&unit.source, alias).trim_start_matches('.'))
            .unwrap_or(imported);
        result.push(Import {
            source: unit.source.clone(),
            namespace: unit.namespace.clone(),
            target: target.clone(),
            object: imported.to_owned(),
            alias: alias.to_owned(),
            span: object.span,
        });
    }
    Ok(result)
}
fn imported_object(
    import: &Import,
    namespaces: &BTreeMap<String, Namespace>,
) -> Result<Symbol, SemanticFailure> {
    let export = namespaces
        .get(&import.target)
        .and_then(|namespace| namespace.objects.get(&import.object))
        .ok_or_else(|| {
            failure(
                &import.source,
                "S2009",
                format!(
                    "unresolved object `.{}` in `{}`",
                    import.object, import.target
                ),
                import.span,
            )
        })?;
    if !visible_from(export, &import.namespace) {
        return Err(failure(
            &import.source,
            "S2010",
            format!("object `.{}` is inaccessible", import.object),
            import.span,
        ));
    }
    Ok(export.clone())
}

fn resolve_imports(
    imports: Vec<Import>,
    namespaces: &mut BTreeMap<String, Namespace>,
) -> Result<(), SemanticFailure> {
    for import in imports {
        let export = imported_object(&import, namespaces)?;
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

fn validate_references(package: &SemanticPackage) -> Result<(), SemanticFailure> {
    fn visit(
        package: &SemanticPackage,
        unit: &SemanticUnit,
        node: &SyntaxNode,
    ) -> Result<(), SemanticFailure> {
        match node.kind {
            SyntaxKind::Name => {
                let name = node_text(&unit.source, node);
                if !matches!(name, "true" | "false")
                    && package
                        .resolve_ordinary_at(unit, node.span.start, name)
                        .is_none()
                {
                    return Err(failure(
                        &unit.source,
                        "S2013",
                        format!("unresolved name `{name}`"),
                        node.span,
                    ));
                }
            }
            SyntaxKind::ObjectName => {
                let name = node_text(&unit.source, node).trim_start_matches('.');
                if package
                    .resolve_object_at(unit, node.span.start, name)
                    .is_none()
                {
                    return Err(failure(
                        &unit.source,
                        "S2014",
                        format!("unresolved object `.{name}`"),
                        node.span,
                    ));
                }
            }
            SyntaxKind::NamespaceDeclaration
            | SyntaxKind::ImportDeclaration
            | SyntaxKind::ParameterList
            | SyntaxKind::Parameter
            | SyntaxKind::TypeExpression
            | SyntaxKind::UnionType
            | SyntaxKind::PrefixType
            | SyntaxKind::AppliedType
            | SyntaxKind::FunctionType => {}
            SyntaxKind::Binding | SyntaxKind::Assignment | SyntaxKind::FunctionDeclaration => {
                let mut declaration_name_skipped = false;
                for child in &node.children {
                    if !declaration_name_skipped
                        && matches!(child.kind, SyntaxKind::Name | SyntaxKind::ObjectName)
                    {
                        declaration_name_skipped = true;
                        continue;
                    }
                    visit(package, unit, child)?;
                }
            }
            SyntaxKind::MemberExpression => {
                if let Some(receiver) = node.children.first() {
                    visit(package, unit, receiver)?;
                }
            }
            _ => {
                for child in &node.children {
                    visit(package, unit, child)?;
                }
            }
        }
        Ok(())
    }

    for unit in &package.units {
        for node in &unit.tree.root.children {
            visit(package, unit, node)?;
        }
    }
    Ok(())
}

fn collect_lexical_scopes(
    unit: &SemanticUnit,
    namespaces: &BTreeMap<String, Namespace>,
) -> Result<Vec<LexicalScope>, SemanticFailure> {
    fn add_scope(
        unit: &SemanticUnit,
        namespaces: &BTreeMap<String, Namespace>,
        scopes: &mut Vec<LexicalScope>,
        node: &SyntaxNode,
        parent: Option<usize>,
        function_body: bool,
    ) -> Result<usize, SemanticFailure> {
        let index = scopes.len();
        scopes.push(LexicalScope {
            span: node.span,
            parent,
            ordinary: BTreeMap::new(),
            objects: BTreeMap::new(),
        });

        if node.kind == SyntaxKind::FunctionDeclaration {
            if let Some(parameters) = node
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::ParameterList)
            {
                for parameter in &parameters.children {
                    if let Some(name) = declaration_name(parameter, &unit.source) {
                        insert_local(unit, scopes, index, name, parameter.span)?;
                    }
                }
            }
        }

        for child in &node.children {
            match child.kind {
                SyntaxKind::ParameterList => {}
                SyntaxKind::Block if node.kind == SyntaxKind::FunctionDeclaration => {
                    populate_scope(unit, namespaces, scopes, index, child)?;
                }
                SyntaxKind::Block => {
                    add_scope(unit, namespaces, scopes, child, Some(index), false)?;
                }
                _ if function_body => {
                    populate_node(unit, namespaces, scopes, index, child)?;
                }
                _ => {}
            }
        }
        Ok(index)
    }

    fn populate_scope(
        unit: &SemanticUnit,
        namespaces: &BTreeMap<String, Namespace>,
        scopes: &mut Vec<LexicalScope>,
        index: usize,
        block: &SyntaxNode,
    ) -> Result<(), SemanticFailure> {
        for node in &block.children {
            populate_node(unit, namespaces, scopes, index, node)?;
        }
        Ok(())
    }

    fn populate_node(
        unit: &SemanticUnit,
        namespaces: &BTreeMap<String, Namespace>,
        scopes: &mut Vec<LexicalScope>,
        index: usize,
        node: &SyntaxNode,
    ) -> Result<(), SemanticFailure> {
        match node.kind {
            SyntaxKind::Binding => {
                if let Some(declaration) = declaration_from_syntax(unit, node)
                    && !declaration.object_form
                    && !declaration.global
                {
                    insert_local(unit, scopes, index, declaration.name, node.span)?;
                }
            }
            SyntaxKind::Assignment => {
                if let Some(declaration) = declaration_from_syntax(unit, node)
                    && !declaration.object_form
                    && !declaration.global
                    && !local_or_namespace_binding_exists(
                        scopes,
                        index,
                        namespaces,
                        &unit.namespace,
                        &declaration.name,
                    )
                {
                    insert_local(unit, scopes, index, declaration.name, node.span)?;
                }
            }
            SyntaxKind::ImportDeclaration => {
                for import in imports_from_syntax(unit, node)? {
                    let export = imported_object(&import, namespaces)?;
                    if let Some(existing) = scopes[index].objects.get(&import.alias) {
                        if existing.identity == export.identity {
                            continue;
                        }
                        return Err(failure(
                            &unit.source,
                            "S2011",
                            format!(
                                "object-form import `.{}` collides; use an alias",
                                import.alias
                            ),
                            import.span,
                        ));
                    }
                    scopes[index].objects.insert(import.alias, export);
                }
            }
            SyntaxKind::FunctionDeclaration => {
                if let Some(name) = declaration_name(node, &unit.source) {
                    insert_local(unit, scopes, index, name, node.span)?;
                }
                add_scope(unit, namespaces, scopes, node, Some(index), true)?;
            }
            SyntaxKind::Block => {
                add_scope(unit, namespaces, scopes, node, Some(index), false)?;
            }
            _ => {
                for child in &node.children {
                    if child.kind == SyntaxKind::Block {
                        add_scope(unit, namespaces, scopes, child, Some(index), false)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn local_or_namespace_binding_exists(
        scopes: &[LexicalScope],
        mut index: usize,
        namespaces: &BTreeMap<String, Namespace>,
        namespace: &str,
        name: &str,
    ) -> bool {
        loop {
            let scope = &scopes[index];
            if scope.ordinary.contains_key(name) {
                return true;
            }
            let Some(parent) = scope.parent else {
                break;
            };
            index = parent;
        }
        namespace_chain(namespace).any(|path| {
            namespaces
                .get(&path)
                .is_some_and(|scope| scope.ordinary.contains_key(name))
        })
    }

    fn insert_local(
        unit: &SemanticUnit,
        scopes: &mut [LexicalScope],
        index: usize,
        name: String,
        span: Span,
    ) -> Result<(), SemanticFailure> {
        let scope = &mut scopes[index];
        if scope.ordinary.contains_key(&name) {
            return Err(failure(
                &unit.source,
                "S2012",
                format!("duplicate binding `{name}` in the same lexical scope"),
                span,
            ));
        }
        scope.ordinary.insert(
            name.clone(),
            Symbol {
                identity: format!("{}::scope{index}::{name}", unit.namespace),
                name,
                namespace: unit.namespace.clone(),
                object_form: false,
                visibility: Visibility::Private,
                global: false,
                kind: SymbolKind::Binding,
                declaration_span: Some(span),
            },
        );
        Ok(())
    }

    let mut scopes = Vec::new();
    for node in &unit.tree.root.children {
        match node.kind {
            SyntaxKind::FunctionDeclaration => {
                add_scope(unit, namespaces, &mut scopes, node, None, true)?;
            }
            SyntaxKind::Block => {
                add_scope(unit, namespaces, &mut scopes, node, None, false)?;
            }
            _ => {}
        }
    }
    Ok(scopes)
}

fn lexical_scope_chain(unit: &SemanticUnit, offset: usize) -> impl Iterator<Item = &LexicalScope> {
    let mut current = unit
        .scopes
        .iter()
        .enumerate()
        .filter(|(_, scope)| scope.span.start <= offset && offset < scope.span.end)
        .min_by_key(|(_, scope)| scope.span.end - scope.span.start)
        .map(|(index, _)| index);
    std::iter::from_fn(move || {
        let index = current?;
        let scope = &unit.scopes[index];
        current = scope.parent;
        Some(scope)
    })
}

fn namespace_chain(namespace: &str) -> impl Iterator<Item = String> {
    let mut current = namespace.trim_end_matches('/').to_owned();
    std::iter::from_fn(move || {
        if current.is_empty() {
            return None;
        }
        let result = current.clone();
        if current == "/" {
            current.clear();
        } else {
            current.truncate(current.rfind('/').unwrap_or(0).max(1));
        }
        Some(result)
    })
}

fn visible_from(symbol: &Symbol, namespace: &str) -> bool {
    match symbol.visibility {
        Visibility::Public => true,
        Visibility::Private => symbol.namespace == namespace,
        Visibility::Protected => {
            symbol.namespace == namespace
                || namespace
                    .strip_prefix(&symbol.namespace)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        }
    }
}

fn bootstrap_prelude() -> BTreeMap<String, Symbol> {
    const PRELUDE: [(&str, &str, &str); 7] = [
        ("print", "/core/output::print", "/core/output"),
        ("int", "/core/types::int", "/core/types"),
        ("float", "/core/types::float", "/core/types"),
        ("bool", "/core/types::bool", "/core/types"),
        ("string", "/core/types::string", "/core/types"),
        ("bytes", "/core/types::bytes", "/core/types"),
        ("none", "/core/types::none", "/core/types"),
    ];
    PRELUDE
        .into_iter()
        .map(|(name, identity, namespace)| {
            (
                name.to_owned(),
                Symbol {
                    identity: identity.to_owned(),
                    name: name.to_owned(),
                    namespace: namespace.to_owned(),
                    object_form: false,
                    visibility: Visibility::Public,
                    global: false,
                    kind: SymbolKind::Binding,
                    declaration_span: None,
                },
            )
        })
        .collect()
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
                    kind: SymbolKind::Binding,
                    declaration_span: None,
                },
            )
        })
        .collect();
    Namespace {
        ordinary: BTreeMap::new(),
        objects,
    }
}

fn normalize_declared_path(components: &[&str]) -> Option<String> {
    if components.is_empty()
        || components
            .iter()
            .any(|component| matches!(*component, "/" | "..") || component.is_empty())
    {
        return None;
    }
    Some(format!("/{}", components.join("/")))
}

fn resolve_path(current: &str, anchored: bool, path: &[&str]) -> Option<String> {
    let mut components = if anchored {
        Vec::new()
    } else {
        current
            .trim_start_matches('/')
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
    };
    for component in path {
        if *component == "/" {
            continue;
        }
        if *component == ".." {
            components.pop()?;
        } else {
            components.push(component);
        }
    }
    Some(format!("/{}", components.join("/")))
}

fn declaration_name(node: &SyntaxNode, source: &SourceFile) -> Option<String> {
    node.children
        .iter()
        .find(|child| matches!(child.kind, SyntaxKind::Name | SyntaxKind::ObjectName))
        .map(|child| node_text(source, child).to_owned())
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
