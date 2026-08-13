use num_bigint::BigInt;
use std::collections::{BTreeMap, BTreeSet};

use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxTree};
use crate::{Diagnostic, Package, ScalarType, SourceFile, Span, lexer, parser};

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
    TypeDescriptor,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueType {
    Scalar(ScalarType),
    ScalarOrNone(ScalarType),
    TypeDescriptor(ScalarType),
}

impl std::fmt::Display for ValueType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scalar(ty) => ty.fmt(formatter),
            Self::ScalarOrNone(ty) => write!(formatter, "{ty}|none"),
            Self::TypeDescriptor(ty) => write!(formatter, "type descriptor `{ty}`"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedBinding {
    pub name: String,
    pub span: Span,
    pub value_type: ValueType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionContract {
    pub name: String,
    pub span: Span,
    pub parameters: Vec<ParameterContract>,
    pub return_type: Option<ScalarType>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterContract {
    pub name: String,
    pub span: Span,
    pub value_type: Option<ScalarType>,
    pub optional: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvaluationKind {
    Call,
    ShortCircuitRhs,
    PostfixUpdate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvaluationStep {
    pub kind: EvaluationKind,
    pub span: Span,
    pub conditional: bool,
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
    pub typed_bindings: Vec<TypedBinding>,
    pub functions: Vec<FunctionContract>,
    pub unreachable_spans: Vec<Span>,
    pub evaluation_steps: Vec<EvaluationStep>,
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
/// Semantic phases fail at the first diagnostic in deterministic package and source
/// order. Unlike independently discoverable manifest errors, later semantic errors can
/// depend on declarations or imports that an earlier error prevented from assembling.
///
/// # Errors
/// Returns the first source-oriented lexer, parser, namespace, scope, or import failure.
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
            typed_bindings: Vec::new(),
            functions: Vec::new(),
            unreachable_spans: Vec::new(),
            evaluation_steps: Vec::new(),
        });
    }

    let mut namespaces = bootstrap_namespaces();
    for unit in &units {
        if matches!(
            unit.namespace.as_str(),
            "/core/output" | "/core/types" | "/core/errors" | "/collections"
        ) {
            let span = unit
                .tree
                .root
                .children
                .iter()
                .find(|node| node.kind == SyntaxKind::NamespaceDeclaration)
                .map_or(Span::new(unit.source.id(), 0, 0), |node| node.span);
            return Err(failure(
                &unit.source,
                "S2017",
                format!(
                    "cannot declare into compiler-owned namespace `{}`",
                    source_namespace(&unit.namespace)
                ),
                span,
            ));
        }
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

    let mut semantic = SemanticPackage {
        identity: package.identity.clone(),
        prelude: package.prelude,
        namespaces,
        globals,
        prelude_bindings,
        units,
        bootstrap_version: BOOTSTRAP_VERSION,
    };
    validate_references(&semantic)?;
    let (typed_units, function_units) = analyze_types(&semantic)?;
    for ((unit, typed_bindings), functions) in semantic
        .units
        .iter_mut()
        .zip(typed_units)
        .zip(function_units)
    {
        unit.typed_bindings = typed_bindings;
        unit.functions = functions;
    }
    validate_calls(&semantic)?;
    validate_definite_assignment(&semantic)?;
    let unreachable_units = validate_control_flow(&semantic)?;
    for (unit, unreachable_spans) in semantic.units.iter_mut().zip(unreachable_units) {
        unit.unreachable_spans = unreachable_spans;
        unit.evaluation_steps = collect_evaluation_steps(&unit.source, &unit.tree.root);
    }
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
            .map_or(imported, |alias| {
                node_text(&unit.source, alias).trim_start_matches('.')
            });
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
                    import.object,
                    source_namespace(&import.target)
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
        let destination = namespaces
            .get_mut(&import.namespace)
            .expect("every import destination is a preassembled source-unit namespace");
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
                if package
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
            | SyntaxKind::ForTarget
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
            SyntaxKind::Argument => {
                for (index, child) in node.children.iter().enumerate() {
                    if index == 0 && node.children.len() > 1 && child.kind == SyntaxKind::Name {
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

fn collect_evaluation_steps(source: &SourceFile, root: &SyntaxNode) -> Vec<EvaluationStep> {
    fn visit(
        source: &SourceFile,
        node: &SyntaxNode,
        conditional: bool,
        steps: &mut Vec<EvaluationStep>,
    ) {
        if node.kind == SyntaxKind::BinaryExpression
            && let [left, right] = node.children.as_slice()
        {
            visit(source, left, conditional, steps);
            let operator = source.text()[left.span.end..right.span.start].trim();
            let short_circuit = matches!(operator, "and" | "or");
            if short_circuit {
                steps.push(EvaluationStep {
                    kind: EvaluationKind::ShortCircuitRhs,
                    span: right.span,
                    conditional: true,
                });
            }
            visit(source, right, conditional || short_circuit, steps);
        } else {
            for child in &node.children {
                visit(source, child, conditional, steps);
            }
        }
        let kind = match node.kind {
            SyntaxKind::CallExpression => Some(EvaluationKind::Call),
            SyntaxKind::PostfixExpression => Some(EvaluationKind::PostfixUpdate),
            _ => None,
        };
        if let Some(kind) = kind {
            steps.push(EvaluationStep {
                kind,
                span: node.span,
                conditional,
            });
        }
    }

    let mut steps = Vec::new();
    visit(source, root, false, &mut steps);
    steps
}

type UnitTypeAnalysis = (Vec<Vec<TypedBinding>>, Vec<Vec<FunctionContract>>);

fn validate_calls(package: &SemanticPackage) -> Result<(), SemanticFailure> {
    for unit in &package.units {
        validate_call_nodes(unit, &unit.tree.root)?;
    }
    Ok(())
}

fn validate_call_nodes(unit: &SemanticUnit, node: &SyntaxNode) -> Result<(), SemanticFailure> {
    if node.kind == SyntaxKind::CallExpression
        && let [callee, arguments] = node.children.as_slice()
        && callee.kind == SyntaxKind::Name
        && let Some(contract) = unit
            .functions
            .iter()
            .find(|contract| contract.name == node_text(&unit.source, callee))
    {
        validate_call_arguments(unit, arguments, contract)?;
    }
    for child in &node.children {
        validate_call_nodes(unit, child)?;
    }
    Ok(())
}

fn validate_call_arguments(
    unit: &SemanticUnit,
    arguments: &SyntaxNode,
    contract: &FunctionContract,
) -> Result<(), SemanticFailure> {
    let mut bound = BTreeSet::new();
    let mut positional = 0;
    let mut named_seen = false;
    for argument in &arguments.children {
        let name = argument
            .children
            .first()
            .filter(|child| child.kind == SyntaxKind::Name && argument.children.len() > 1);
        let parameter = if let Some(name) = name {
            named_seen = true;
            let name_text = node_text(&unit.source, name);
            contract
                .parameters
                .iter()
                .find(|parameter| parameter.name == name_text)
                .ok_or_else(|| {
                    failure(
                        &unit.source,
                        "T0012",
                        format!(
                            "function `{}` has no parameter named `{name_text}`",
                            contract.name
                        ),
                        name.span,
                    )
                })?
        } else {
            if named_seen {
                return Err(failure(
                    &unit.source,
                    "T0012",
                    "positional arguments must precede named arguments",
                    argument.span,
                ));
            }
            let parameter = contract.parameters.get(positional).ok_or_else(|| {
                failure(
                    &unit.source,
                    "T0012",
                    format!("too many arguments for function `{}`", contract.name),
                    argument.span,
                )
            })?;
            positional += 1;
            parameter
        };
        if !bound.insert(parameter.name.as_str()) {
            return Err(failure(
                &unit.source,
                "T0012",
                format!("parameter `{}` is bound more than once", parameter.name),
                argument.span,
            ));
        }
    }
    if let Some(missing) = contract
        .parameters
        .iter()
        .find(|parameter| !parameter.optional && !bound.contains(parameter.name.as_str()))
    {
        return Err(failure(
            &unit.source,
            "T0012",
            format!("missing required argument `{}`", missing.name),
            arguments.span,
        ));
    }
    Ok(())
}

fn analyze_types(package: &SemanticPackage) -> Result<UnitTypeAnalysis, SemanticFailure> {
    let mut binding_units = Vec::with_capacity(package.units.len());
    let mut function_units = Vec::with_capacity(package.units.len());
    for unit in &package.units {
        let mut aliases = BTreeMap::new();
        if package.prelude {
            for ty in ScalarType::ALL {
                aliases.insert(ty.source_name().to_owned(), ty);
            }
        }
        let mut bindings = Vec::new();
        let mut functions = Vec::new();
        analyze_type_nodes(
            package,
            unit,
            &unit.tree.root,
            &mut aliases,
            &mut bindings,
            &mut functions,
        )?;
        binding_units.push(bindings);
        function_units.push(functions);
    }
    Ok((binding_units, function_units))
}

fn analyze_type_nodes(
    package: &SemanticPackage,
    unit: &SemanticUnit,
    node: &SyntaxNode,
    aliases: &mut BTreeMap<String, ScalarType>,
    bindings: &mut Vec<TypedBinding>,
    functions: &mut Vec<FunctionContract>,
) -> Result<(), SemanticFailure> {
    if matches!(node.kind, SyntaxKind::Binding | SyntaxKind::Assignment) {
        analyze_binding_node(package, unit, node, aliases, bindings)?;
    } else if node.kind == SyntaxKind::FunctionDeclaration {
        functions.push(analyze_function_contract(unit, node, aliases)?);
    }
    for child in &node.children {
        analyze_type_nodes(package, unit, child, aliases, bindings, functions)?;
    }
    Ok(())
}

fn analyze_function_contract(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    aliases: &BTreeMap<String, ScalarType>,
) -> Result<FunctionContract, SemanticFailure> {
    let name_node = node
        .children
        .iter()
        .find(|child| child.kind == SyntaxKind::Name)
        .ok_or_else(|| failure(&unit.source, "T0004", "function requires a name", node.span))?;
    let return_type = node
        .children
        .iter()
        .find(|child| child.kind == SyntaxKind::TypeExpression)
        .map(|type_node| resolve_scalar_type(&unit.source, type_node, aliases))
        .transpose()?;
    let mut parameters = Vec::new();
    let mut optional_seen = false;
    if let Some(parameter_list) = node
        .children
        .iter()
        .find(|child| child.kind == SyntaxKind::ParameterList)
    {
        for parameter in &parameter_list.children {
            let Some(parameter_name) = parameter
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Name)
            else {
                continue;
            };
            let type_node = parameter
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::TypeExpression);
            let value_type = type_node
                .map(|node| resolve_scalar_type(&unit.source, node, aliases))
                .transpose()?;
            let default = parameter.children.iter().rev().find(|child| {
                child.span != parameter_name.span && child.kind != SyntaxKind::TypeExpression
            });
            let optional = default.is_some();
            if optional {
                optional_seen = true;
            } else if optional_seen {
                return Err(failure(
                    &unit.source,
                    "T0005",
                    "required parameters must precede optional parameters",
                    parameter.span,
                ));
            }
            if let (Some(expected), Some(default)) = (value_type, default)
                && let Some(actual) = infer_literal_type(unit, default)
                && actual != expected
            {
                if actual == ScalarType::Int
                    && expected.is_integer()
                    && let Some(value) = constant_integer(unit, default)
                {
                    check_integer_range(&unit.source, expected, &value, default.span)?;
                } else {
                    return Err(failure(
                        &unit.source,
                        "T0006",
                        format!(
                            "default for parameter `{}` has type `{actual}`, expected `{expected}`",
                            node_text(&unit.source, parameter_name)
                        ),
                        default.span,
                    ));
                }
            }
            parameters.push(ParameterContract {
                name: node_text(&unit.source, parameter_name).to_owned(),
                span: parameter.span,
                value_type,
                optional,
            });
        }
    }
    Ok(FunctionContract {
        name: node_text(&unit.source, name_node).to_owned(),
        span: node.span,
        parameters,
        return_type,
    })
}

fn resolve_scalar_type(
    source: &SourceFile,
    type_node: &SyntaxNode,
    aliases: &BTreeMap<String, ScalarType>,
) -> Result<ScalarType, SemanticFailure> {
    let name = node_text(source, type_node).trim();
    aliases.get(name).copied().ok_or_else(|| {
        failure(
            source,
            "T0001",
            format!("`{name}` does not resolve to a scalar type descriptor"),
            type_node.span,
        )
    })
}

fn analyze_binding_node(
    package: &SemanticPackage,
    unit: &SemanticUnit,
    node: &SyntaxNode,
    aliases: &mut BTreeMap<String, ScalarType>,
    bindings: &mut Vec<TypedBinding>,
) -> Result<(), SemanticFailure> {
    let Some(name_node) = node
        .children
        .iter()
        .find(|child| matches!(child.kind, SyntaxKind::Name | SyntaxKind::ObjectName))
    else {
        return Ok(());
    };
    if name_node.kind == SyntaxKind::ObjectName {
        return Ok(());
    }
    let name = node_text(&unit.source, name_node).to_owned();
    let declared = node
        .children
        .iter()
        .find(|child| child.kind == SyntaxKind::TypeExpression);
    let initializer = node.children.iter().rev().find(|child| {
        child.span != name_node.span
            && !matches!(
                child.kind,
                SyntaxKind::DeclarationModifier
                    | SyntaxKind::Visibility
                    | SyntaxKind::DeclarationQualifier
                    | SyntaxKind::TypeExpression
            )
    });

    if declared.is_none()
        && let Some(initializer) = initializer
        && initializer.kind == SyntaxKind::ObjectName
    {
        let object_name = node_text(&unit.source, initializer).trim_start_matches('.');
        if let Some(symbol) = package.resolve_object_at(unit, initializer.span.start, object_name)
            && symbol.kind == SymbolKind::TypeDescriptor
            && let Some(ty) = descriptor_scalar(symbol)
        {
            aliases.insert(name.clone(), ty);
            bindings.push(TypedBinding {
                name,
                span: node.span,
                value_type: ValueType::TypeDescriptor(ty),
            });
        }
        return Ok(());
    }

    if node.kind == SyntaxKind::Assignment
        && declared.is_none()
        && let Some(previous) = bindings.iter().rev().find(|binding| binding.name == name)
        && let ValueType::Scalar(expected) = previous.value_type
        && let Some(initializer) = initializer
        && let Some(actual) = infer_value_type(unit, initializer, aliases, bindings)?
    {
        validate_value_assignment(&unit.source, &name, expected, actual, initializer)?;
        return Ok(());
    }
    let inferred = initializer
        .map(|value| infer_value_type(unit, value, aliases, bindings))
        .transpose()?
        .flatten();
    let value_type = if let Some(type_node) = declared {
        let type_name = node_text(&unit.source, type_node).trim();
        let Some(ty) = aliases.get(type_name).copied() else {
            return Err(failure(
                &unit.source,
                "T0001",
                format!("`{type_name}` does not resolve to a scalar type descriptor"),
                type_node.span,
            ));
        };
        if let Some(inferred) = inferred
            && inferred != ValueType::Scalar(ty)
            && let Some(initializer) = initializer
        {
            validate_value_assignment(&unit.source, &name, ty, inferred, initializer)?;
        }
        ValueType::Scalar(ty)
    } else if let Some(inferred) = inferred {
        inferred
    } else {
        return Ok(());
    };

    bindings.push(TypedBinding {
        name,
        span: node.span,
        value_type,
    });
    Ok(())
}

fn validate_value_assignment(
    source: &SourceFile,
    name: &str,
    expected: ScalarType,
    actual: ValueType,
    value: &SyntaxNode,
) -> Result<(), SemanticFailure> {
    if let ValueType::Scalar(actual) = actual {
        if actual == expected {
            return Ok(());
        }
        if actual == ScalarType::Int
            && expected.is_integer()
            && let Some(integer) = constant_integer_from_source(source, value)
        {
            return check_integer_range(source, expected, &integer, value.span);
        }
    }
    Err(failure(
        source,
        "T0002",
        format!("cannot assign `{actual}` to `{name}` of type `{expected}`"),
        value.span,
    ))
}

fn constant_integer_from_source(source: &SourceFile, node: &SyntaxNode) -> Option<BigInt> {
    if node.kind == SyntaxKind::UnaryExpression {
        let value = node.children.last()?;
        let magnitude = constant_integer_from_source(source, value)?;
        return match &source.text()[node.span.start..value.span.start] {
            prefix if prefix.trim() == "-" => Some(-magnitude),
            prefix if prefix.trim() == "+" => Some(magnitude),
            _ => None,
        };
    }
    (node.kind == SyntaxKind::Literal)
        .then(|| source.text()[node.span.start..node.span.end].replace('_', ""))
        .and_then(|text| BigInt::parse_bytes(text.as_bytes(), 10))
}
fn infer_value_type(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    aliases: &BTreeMap<String, ScalarType>,
    bindings: &[TypedBinding],
) -> Result<Option<ValueType>, SemanticFailure> {
    if node.kind == SyntaxKind::Literal {
        return Ok(infer_literal_type(unit, node).map(ValueType::Scalar));
    }
    if node.kind == SyntaxKind::GroupExpression {
        return match node.children.first() {
            Some(child) => infer_value_type(unit, child, aliases, bindings),
            None => Ok(None),
        };
    }
    if node.kind == SyntaxKind::UnaryExpression {
        return infer_unary_type(unit, node, aliases, bindings).map(Some);
    }
    if node.kind == SyntaxKind::BinaryExpression {
        return infer_binary_type(unit, node, aliases, bindings).map(Some);
    }
    if node.kind == SyntaxKind::TypeMembershipExpression {
        return Ok(Some(ValueType::Scalar(ScalarType::Bool)));
    }
    if node.kind == SyntaxKind::Name {
        let name = node_text(&unit.source, node);
        return Ok(bindings
            .iter()
            .rev()
            .find(|binding| binding.name == name)
            .map(|binding| binding.value_type));
    }
    if node.kind == SyntaxKind::CallExpression {
        return infer_integer_coercion_type(unit, node, aliases, bindings);
    }
    Ok(None)
}

fn infer_unary_type(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    aliases: &BTreeMap<String, ScalarType>,
    bindings: &[TypedBinding],
) -> Result<ValueType, SemanticFailure> {
    let Some(operand_node) = node.children.last() else {
        return Err(operator_failure(
            unit,
            node,
            "unary operator requires an operand",
        ));
    };
    let Some(ValueType::Scalar(operand)) = infer_value_type(unit, operand_node, aliases, bindings)?
    else {
        return Err(operator_failure(
            unit,
            node,
            "unary operator requires a scalar operand",
        ));
    };
    let operator = unit.source.text()[node.span.start..operand_node.span.start].trim();
    let valid = match operator {
        "-" => {
            operand.is_integer()
                || matches!(
                    operand,
                    ScalarType::Float | ScalarType::Float32 | ScalarType::Float64
                )
        }
        "~" => operand.is_integer(),
        "not" => operand == ScalarType::Bool,
        _ => false,
    };
    if !valid {
        return Err(operator_failure(
            unit,
            node,
            format!("operator `{operator}` is not defined for `{operand}`"),
        ));
    }
    Ok(ValueType::Scalar(if operator == "not" {
        ScalarType::Bool
    } else {
        operand
    }))
}

fn infer_integer_coercion_type(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    aliases: &BTreeMap<String, ScalarType>,
    bindings: &[TypedBinding],
) -> Result<Option<ValueType>, SemanticFailure> {
    let Some(member) = node.children.first() else {
        return Ok(None);
    };
    if member.kind != SyntaxKind::MemberExpression {
        return Ok(None);
    }
    let Some(operation_node) = member.children.get(1) else {
        return Ok(None);
    };
    let operation = node_text(&unit.source, operation_node);
    if !matches!(
        operation,
        "coerce" | "checked-coerce" | "wrapping-coerce" | "saturating-coerce"
    ) {
        return Ok(None);
    }
    let source_node = &member.children[0];
    let Some(ValueType::Scalar(source_type)) =
        infer_value_type(unit, source_node, aliases, bindings)?
    else {
        return Err(failure(
            &unit.source,
            "T0009",
            format!("`{operation}` requires an integer source"),
            source_node.span,
        ));
    };
    if !source_type.is_integer() {
        return Err(failure(
            &unit.source,
            "T0009",
            format!("`{operation}` requires an integer source"),
            source_node.span,
        ));
    }
    let destination_node = node
        .children
        .get(1)
        .and_then(|arguments| arguments.children.first())
        .and_then(|argument| argument.children.last())
        .ok_or_else(|| {
            failure(
                &unit.source,
                "T0008",
                format!("`{operation}` requires one integer destination"),
                node.span,
            )
        })?;
    let destination_name = node_text(&unit.source, destination_node);
    let destination = aliases.get(destination_name).copied().ok_or_else(|| {
        failure(
            &unit.source,
            "T0008",
            format!("`{destination_name}` is not a supported integer coercion destination"),
            destination_node.span,
        )
    })?;
    if !destination.is_integer() {
        return Err(failure(
            &unit.source,
            "T0008",
            format!("`{destination}` is not a supported integer coercion destination"),
            destination_node.span,
        ));
    }
    if destination == ScalarType::Int
        && matches!(operation, "wrapping-coerce" | "saturating-coerce")
    {
        return Err(failure(
            &unit.source,
            "T0010",
            format!("`{operation}` requires a fixed-width integer destination"),
            destination_node.span,
        ));
    }
    let result = if operation == "checked-coerce" {
        ValueType::ScalarOrNone(destination)
    } else {
        ValueType::Scalar(destination)
    };
    Ok(Some(result))
}
fn infer_binary_type(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    aliases: &BTreeMap<String, ScalarType>,
    bindings: &[TypedBinding],
) -> Result<ValueType, SemanticFailure> {
    let [left_node, right_node] = node.children.as_slice() else {
        return Err(operator_failure(
            unit,
            node,
            "binary operator requires two operands",
        ));
    };
    let left = infer_value_type(unit, left_node, aliases, bindings)?;
    let right = infer_value_type(unit, right_node, aliases, bindings)?;
    let operator = unit.source.text()[left_node.span.end..right_node.span.start].trim();
    if operator == "is" {
        return Ok(ValueType::Scalar(ScalarType::Bool));
    }
    let (Some(ValueType::Scalar(left)), Some(ValueType::Scalar(right))) = (left, right) else {
        return Err(operator_failure(
            unit,
            node,
            "operator requires scalar operands",
        ));
    };
    let same = left == right;
    let numeric = |ty: ScalarType| {
        ty.is_integer()
            || matches!(
                ty,
                ScalarType::Float | ScalarType::Float32 | ScalarType::Float64
            )
    };
    let result = match operator {
        "+" | "-" | "*" | "/" | "%" if same && numeric(left) => left,
        "<<" | ">>" if left.is_integer() && right.is_integer() => left,
        "&" | "^" | "|" if same && left.is_integer() => left,
        "and" | "or" if left == ScalarType::Bool && right == ScalarType::Bool => ScalarType::Bool,
        "==" | "!=" if same => ScalarType::Bool,
        "<" | "<=" | ">" | ">=" if same && (numeric(left) || left == ScalarType::String) => {
            ScalarType::Bool
        }
        _ => {
            return Err(operator_failure(
                unit,
                node,
                format!("operator `{operator}` is not defined for `{left}` and `{right}`"),
            ));
        }
    };
    Ok(ValueType::Scalar(result))
}

fn operator_failure(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    message: impl Into<String>,
) -> SemanticFailure {
    failure(&unit.source, "T0011", message, node.span)
}

fn descriptor_scalar(symbol: &Symbol) -> Option<ScalarType> {
    symbol
        .identity
        .strip_prefix("/core/types::")
        .and_then(ScalarType::from_source_name)
}

fn infer_literal_type(unit: &SemanticUnit, node: &SyntaxNode) -> Option<ScalarType> {
    if node.kind == SyntaxKind::UnaryExpression {
        return node
            .children
            .last()
            .and_then(|child| infer_literal_type(unit, child));
    }
    if node.kind != SyntaxKind::Literal {
        return None;
    }
    let text = node_text(&unit.source, node);
    match text {
        "true" | "false" => Some(ScalarType::Bool),
        value if value.starts_with(['\'', '"', '>']) => Some(ScalarType::String),
        _ => Some(ScalarType::Int),
    }
}

fn constant_integer(unit: &SemanticUnit, node: &SyntaxNode) -> Option<BigInt> {
    let compact = node_text(&unit.source, node)
        .chars()
        .filter(|character| !character.is_whitespace() && *character != '_')
        .collect::<String>();
    let (negative, digits) = compact
        .strip_prefix('-')
        .map_or((false, compact.as_str()), |digits| (true, digits));
    let (radix, digits) = if let Some(digits) = digits.strip_prefix("0x") {
        (16, digits)
    } else if let Some(digits) = digits.strip_prefix("0o") {
        (8, digits)
    } else if let Some(digits) = digits.strip_prefix("0b") {
        (2, digits)
    } else {
        (10, digits)
    };
    let value = BigInt::parse_bytes(digits.as_bytes(), radix)?;
    Some(if negative { -value } else { value })
}

fn check_integer_range(
    source: &SourceFile,
    destination: ScalarType,
    value: &BigInt,
    span: Span,
) -> Result<(), SemanticFailure> {
    let bounds = match destination {
        ScalarType::Int8 => integer_bounds(8, true),
        ScalarType::Int16 => integer_bounds(16, true),
        ScalarType::Int32 => integer_bounds(32, true),
        ScalarType::Int64 => integer_bounds(64, true),
        ScalarType::Int128 => integer_bounds(128, true),
        ScalarType::Uint8 => integer_bounds(8, false),
        ScalarType::Uint16 => integer_bounds(16, false),
        ScalarType::Uint32 => integer_bounds(32, false),
        ScalarType::Uint64 => integer_bounds(64, false),
        ScalarType::Uint128 => integer_bounds(128, false),
        _ => return Ok(()),
    };
    if value < &bounds.0 || value > &bounds.1 {
        return Err(failure(
            source,
            "T0003",
            format!("constant `{value}` is outside the range of `{destination}`"),
            span,
        ));
    }
    Ok(())
}

fn integer_bounds(bits: usize, signed: bool) -> (BigInt, BigInt) {
    if signed {
        let magnitude = BigInt::from(1_u8) << (bits - 1);
        (-&magnitude, magnitude - 1)
    } else {
        (BigInt::from(0_u8), (BigInt::from(1_u8) << bits) - 1)
    }
}

fn collect_lexical_scopes(
    unit: &SemanticUnit,
    namespaces: &BTreeMap<String, Namespace>,
) -> Result<Vec<LexicalScope>, SemanticFailure> {
    let mut scopes = Vec::new();
    for node in &unit.tree.root.children {
        match node.kind {
            SyntaxKind::FunctionDeclaration => {
                add_lexical_scope(unit, namespaces, &mut scopes, node, None, true)?;
            }
            SyntaxKind::Block => {
                add_lexical_scope(unit, namespaces, &mut scopes, node, None, false)?;
            }
            _ => {}
        }
    }
    Ok(scopes)
}

fn add_lexical_scope(
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
    if node.kind == SyntaxKind::FunctionDeclaration
        && let Some(parameters) = node
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
    for child in &node.children {
        match child.kind {
            SyntaxKind::ParameterList => {}
            SyntaxKind::Block if node.kind == SyntaxKind::FunctionDeclaration => {
                populate_scope(unit, namespaces, scopes, index, child)?;
            }
            SyntaxKind::Block => {
                add_lexical_scope(unit, namespaces, scopes, child, Some(index), false)?;
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
            populate_imports(unit, namespaces, scopes, index, node)?;
        }
        SyntaxKind::FunctionDeclaration => {
            if let Some(name) = declaration_name(node, &unit.source) {
                insert_local(unit, scopes, index, name, node.span)?;
            }
            add_lexical_scope(unit, namespaces, scopes, node, Some(index), true)?;
        }
        SyntaxKind::Block => {
            add_lexical_scope(unit, namespaces, scopes, node, Some(index), false)?;
        }
        _ => {
            for child in &node.children {
                if child.kind == SyntaxKind::Block {
                    add_lexical_scope(unit, namespaces, scopes, child, Some(index), false)?;
                }
            }
        }
    }
    Ok(())
}
fn validate_definite_assignment(package: &SemanticPackage) -> Result<(), SemanticFailure> {
    for unit in &package.units {
        for function in unit
            .tree
            .root
            .children
            .iter()
            .filter(|node| node.kind == SyntaxKind::FunctionDeclaration)
        {
            let mut declared = BTreeSet::new();
            let mut assigned = BTreeSet::new();
            if let Some(parameters) = function
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::ParameterList)
            {
                for parameter in &parameters.children {
                    if let Some(name) = parameter
                        .children
                        .iter()
                        .find(|child| child.kind == SyntaxKind::Name)
                    {
                        assigned.insert(node_text(&unit.source, name).to_owned());
                    }
                }
            }
            if let Some(block) = function
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Block)
            {
                validate_assignment_block(unit, block, &mut declared, &mut assigned)?;
            }
        }
    }
    Ok(())
}

fn validate_assignment_block(
    unit: &SemanticUnit,
    block: &SyntaxNode,
    declared: &mut BTreeSet<String>,
    assigned: &mut BTreeSet<String>,
) -> Result<(), SemanticFailure> {
    for statement in &block.children {
        match statement.kind {
            SyntaxKind::Binding => {
                let name_node = statement
                    .children
                    .iter()
                    .find(|child| child.kind == SyntaxKind::Name);
                let Some(name_node) = name_node else {
                    continue;
                };
                let name = node_text(&unit.source, name_node).to_owned();
                let initializer = statement.children.iter().rev().find(|child| {
                    child.span != name_node.span && child.kind != SyntaxKind::TypeExpression
                });
                if let Some(initializer) = initializer {
                    validate_assigned_reads(unit, initializer, declared, assigned)?;
                    assigned.insert(name.clone());
                }
                if statement
                    .children
                    .iter()
                    .any(|child| child.kind == SyntaxKind::TypeExpression)
                {
                    declared.insert(name);
                }
            }
            SyntaxKind::Assignment => {
                if let Some(value) = statement.children.get(1) {
                    validate_assigned_reads(unit, value, declared, assigned)?;
                }
                if let Some(target) = statement.children.first()
                    && target.kind == SyntaxKind::Name
                {
                    assigned.insert(node_text(&unit.source, target).to_owned());
                }
            }
            SyntaxKind::IfStatement => {
                if let Some(condition) = statement.children.first() {
                    validate_assigned_reads(unit, condition, declared, assigned)?;
                }
                let incoming = assigned.clone();
                let mut branch_results = Vec::new();
                for branch in statement.children.iter().skip(1) {
                    let branch_block = if branch.kind == SyntaxKind::Block {
                        Some(branch)
                    } else {
                        branch
                            .children
                            .iter()
                            .find(|child| child.kind == SyntaxKind::Block)
                    };
                    if let Some(branch_block) = branch_block {
                        let mut branch_assigned = incoming.clone();
                        validate_assignment_block(
                            unit,
                            branch_block,
                            declared,
                            &mut branch_assigned,
                        )?;
                        branch_results.push(branch_assigned);
                    }
                }
                let has_else = statement
                    .children
                    .iter()
                    .any(|child| child.kind == SyntaxKind::ElseClause);
                if !has_else {
                    branch_results.push(incoming);
                }
                if let Some(first) = branch_results.first() {
                    *assigned = branch_results
                        .iter()
                        .skip(1)
                        .fold(first.clone(), |common, branch| {
                            common.intersection(branch).cloned().collect()
                        });
                }
            }
            _ => validate_assigned_reads(unit, statement, declared, assigned)?,
        }
    }
    Ok(())
}

fn validate_assigned_reads(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    declared: &BTreeSet<String>,
    assigned: &BTreeSet<String>,
) -> Result<(), SemanticFailure> {
    if node.kind == SyntaxKind::Name {
        let name = node_text(&unit.source, node);
        if declared.contains(name) && !assigned.contains(name) {
            return Err(failure(
                &unit.source,
                "T0007",
                format!("`{name}` may be read before it is assigned"),
                node.span,
            ));
        }
    }
    for child in &node.children {
        validate_assigned_reads(unit, child, declared, assigned)?;
    }
    Ok(())
}

fn validate_control_flow(package: &SemanticPackage) -> Result<Vec<Vec<Span>>, SemanticFailure> {
    let mut unreachable_units = Vec::with_capacity(package.units.len());
    for unit in &package.units {
        let mut unreachable = Vec::new();
        for function in unit
            .tree
            .root
            .children
            .iter()
            .filter(|node| node.kind == SyntaxKind::FunctionDeclaration)
        {
            let Some(name_node) = function
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Name)
            else {
                continue;
            };
            let Some(contract) = unit
                .functions
                .iter()
                .find(|contract| contract.name == node_text(&unit.source, name_node))
            else {
                continue;
            };
            let Some(block) = function
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Block)
            else {
                continue;
            };
            let mut bindings = unit.typed_bindings.clone();
            bindings.extend(contract.parameters.iter().filter_map(|parameter| {
                parameter.value_type.map(|value_type| TypedBinding {
                    name: parameter.name.clone(),
                    span: parameter.span,
                    value_type: ValueType::Scalar(value_type),
                })
            }));
            let falls_through =
                validate_flow_block(unit, block, contract, &bindings, 0, &mut unreachable)?;
            if contract.return_type.is_some() && falls_through {
                return Err(failure(
                    &unit.source,
                    "T0015",
                    format!(
                        "function `{}` may finish without returning a value",
                        contract.name
                    ),
                    function.span,
                ));
            }
        }
        unreachable_units.push(unreachable);
    }
    Ok(unreachable_units)
}

fn validate_flow_block(
    unit: &SemanticUnit,
    block: &SyntaxNode,
    contract: &FunctionContract,
    bindings: &[TypedBinding],
    loop_depth: usize,
    unreachable: &mut Vec<Span>,
) -> Result<bool, SemanticFailure> {
    let mut falls_through = true;
    for statement in &block.children {
        if !falls_through {
            unreachable.push(statement.span);
            continue;
        }
        falls_through =
            validate_flow_statement(unit, statement, contract, bindings, loop_depth, unreachable)?;
    }
    Ok(falls_through)
}

fn validate_flow_statement(
    unit: &SemanticUnit,
    statement: &SyntaxNode,
    contract: &FunctionContract,
    bindings: &[TypedBinding],
    loop_depth: usize,
    unreachable: &mut Vec<Span>,
) -> Result<bool, SemanticFailure> {
    match statement.kind {
        SyntaxKind::ReturnStatement => {
            validate_return(unit, statement, contract, bindings)?;
            Ok(false)
        }
        SyntaxKind::BreakStatement | SyntaxKind::ContinueStatement => {
            if loop_depth == 0 {
                let keyword = node_text(&unit.source, statement);
                return Err(failure(
                    &unit.source,
                    "T0014",
                    format!("`{keyword}` is only valid inside a loop"),
                    statement.span,
                ));
            }
            Ok(false)
        }
        SyntaxKind::IfStatement => {
            validate_if_flow(unit, statement, contract, bindings, loop_depth, unreachable)
        }
        SyntaxKind::WhileStatement => {
            if let Some(condition) = statement.children.first() {
                validate_bool_condition(unit, condition, bindings)?;
            }
            if let Some(block) = statement
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Block)
            {
                validate_flow_block(unit, block, contract, bindings, loop_depth + 1, unreachable)?;
            }
            Ok(true)
        }
        SyntaxKind::ForStatement => {
            if statement.children.len() == 4 {
                validate_bool_condition(unit, &statement.children[1], bindings)?;
            } else if let [target, collection, _block] = statement.children.as_slice() {
                let collection_type =
                    infer_value_type(unit, collection, &BTreeMap::new(), bindings)?;
                if collection_type != Some(ValueType::Scalar(ScalarType::String)) {
                    return Err(failure(
                        &unit.source,
                        "T0016",
                        "version-one collection iteration supports `string` only",
                        collection.span,
                    ));
                }
                if target.children.len() != 1 {
                    return Err(failure(
                        &unit.source,
                        "T0016",
                        "string iteration requires exactly one target",
                        target.span,
                    ));
                }
            }
            if let Some(block) = statement
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Block)
            {
                validate_flow_block(unit, block, contract, bindings, loop_depth + 1, unreachable)?;
            }
            Ok(true)
        }
        SyntaxKind::PostfixExpression => {
            let Some(operand) = statement.children.first() else {
                return Ok(true);
            };
            if operand.kind != SyntaxKind::Name
                || !matches!(
                    infer_value_type(unit, operand, &BTreeMap::new(), bindings)?,
                    Some(ValueType::Scalar(ty)) if ty.is_integer()
                )
            {
                return Err(failure(
                    &unit.source,
                    "T0014",
                    "postfix update requires an assignable integer binding",
                    statement.span,
                ));
            }
            Ok(true)
        }
        _ => Ok(true),
    }
}

fn validate_bool_condition(
    unit: &SemanticUnit,
    condition: &SyntaxNode,
    bindings: &[TypedBinding],
) -> Result<(), SemanticFailure> {
    if matches!(
        infer_value_type(unit, condition, &BTreeMap::new(), bindings)?,
        Some(ValueType::Scalar(ScalarType::Bool))
    ) {
        return Ok(());
    }
    Err(failure(
        &unit.source,
        "T0014",
        "control-flow condition must have type `bool`",
        condition.span,
    ))
}

fn validate_if_flow(
    unit: &SemanticUnit,
    statement: &SyntaxNode,
    contract: &FunctionContract,
    bindings: &[TypedBinding],
    loop_depth: usize,
    unreachable: &mut Vec<Span>,
) -> Result<bool, SemanticFailure> {
    let condition = statement.children.first().ok_or_else(|| {
        failure(
            &unit.source,
            "T0014",
            "an `if` statement requires a condition",
            statement.span,
        )
    })?;
    validate_bool_condition(unit, condition, bindings)?;
    let mut branch_falls_through = Vec::new();
    let mut has_else = false;
    for branch in statement.children.iter().skip(1) {
        let block = if branch.kind == SyntaxKind::Block {
            Some(branch)
        } else if branch.kind == SyntaxKind::ElseClause {
            let mut children = branch.children.iter();
            let first = children.next();
            if first.is_some_and(|child| child.kind == SyntaxKind::Block) {
                has_else = true;
                first
            } else {
                if let Some(condition) = first {
                    validate_bool_condition(unit, condition, bindings)?;
                }
                children.find(|child| child.kind == SyntaxKind::Block)
            }
        } else {
            None
        };
        if let Some(block) = block {
            branch_falls_through.push(validate_flow_block(
                unit,
                block,
                contract,
                bindings,
                loop_depth,
                unreachable,
            )?);
        }
    }
    Ok(!has_else || branch_falls_through.into_iter().any(|branch| branch))
}

fn validate_return(
    unit: &SemanticUnit,
    statement: &SyntaxNode,
    contract: &FunctionContract,
    bindings: &[TypedBinding],
) -> Result<(), SemanticFailure> {
    let value = statement.children.first();
    match (contract.return_type, value) {
        (None, None) => Ok(()),
        (None, Some(value)) => Err(failure(
            &unit.source,
            "T0015",
            format!("function `{}` does not return a value", contract.name),
            value.span,
        )),
        (Some(expected), None) => Err(failure(
            &unit.source,
            "T0015",
            format!("function `{}` must return `{expected}`", contract.name),
            statement.span,
        )),
        (Some(expected), Some(value)) => {
            let actual = infer_value_type(unit, value, &BTreeMap::new(), bindings)?;
            if actual == Some(ValueType::Scalar(expected)) {
                Ok(())
            } else {
                Err(failure(
                    &unit.source,
                    "T0015",
                    format!("function `{}` must return `{expected}`", contract.name),
                    value.span,
                ))
            }
        }
    }
}

fn populate_imports(
    unit: &SemanticUnit,
    namespaces: &BTreeMap<String, Namespace>,
    scopes: &mut [LexicalScope],
    index: usize,
    node: &SyntaxNode,
) -> Result<(), SemanticFailure> {
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

fn source_namespace(namespace: &str) -> String {
    namespace.strip_prefix('/').map_or_else(
        || namespace.replace('/', " "),
        |components| {
            if components.is_empty() {
                "/".to_owned()
            } else {
                format!("/{}", components.replace('/', " "))
            }
        },
    )
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
                    kind: if name == "print" {
                        SymbolKind::Binding
                    } else {
                        SymbolKind::TypeDescriptor
                    },
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
        namespace_with_objects("/core/output", ["print"], SymbolKind::Binding),
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
        namespace_with_objects(
            "/core/types",
            types.iter().map(String::as_str),
            SymbolKind::TypeDescriptor,
        ),
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
            SymbolKind::Binding,
        ),
    );
    namespaces.insert("/collections".to_owned(), Namespace::default());
    namespaces
}

fn namespace_with_objects<'a>(
    path: &str,
    names: impl IntoIterator<Item = &'a str>,
    kind: SymbolKind,
) -> Namespace {
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
                    kind,
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
