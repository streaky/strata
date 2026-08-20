use std::collections::{BTreeMap, BTreeSet};

use num_bigint::BigInt;
use num_traits::{FromPrimitive, ToPrimitive};

use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxTree};
use crate::{Diagnostic, Package, ScalarType, SourceFile, Span, TypeCategory, lexer, parser};

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
    Interface,
    ErrorObject,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Symbol {
    pub identity: String,
    pub name: String,
    pub namespace: String,
    pub visibility: Visibility,
    pub global: bool,
    pub constant: bool,
    pub kind: SymbolKind,
    pub declaration_span: Option<Span>,
}

impl Symbol {
    /// Returns the compiler-owned scalar represented by this canonical type descriptor.
    #[must_use]
    pub fn descriptor_type(&self) -> Option<ScalarType> {
        (self.kind == SymbolKind::TypeDescriptor)
            .then(|| self.identity.strip_prefix("/core/types::"))
            .flatten()
            .and_then(ScalarType::from_source_name)
    }

    #[must_use]
    pub fn descriptor_category(&self) -> Option<TypeCategory> {
        (self.kind == SymbolKind::TypeDescriptor)
            .then(|| self.identity.strip_prefix("/core/types::"))
            .flatten()
            .and_then(TypeCategory::from_source_name)
    }

    #[must_use]
    pub fn available_in_function_body(&self) -> bool {
        self.kind != SymbolKind::Binding || self.constant || self.global
    }
}

#[derive(Clone, Debug, Default)]
pub struct Namespace {
    pub symbols: BTreeMap<String, Symbol>,
}

#[derive(Clone, Debug)]
pub struct SemanticPackage {
    pub identity: String,
    pub prelude: bool,
    pub namespaces: BTreeMap<String, Namespace>,
    pub globals: BTreeMap<String, Symbol>,
    pub prelude_bindings: BTreeMap<String, Symbol>,
    pub descriptor_constructs: BTreeMap<String, Symbol>,
    pub units: Vec<SemanticUnit>,
    pub bootstrap_version: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueType {
    Scalar(ScalarType),
    ScalarOrNone(ScalarType),
    OverflowResult(ScalarType),
    DivRemResult(ScalarType),
    StringView(TextUnit),
    StringList,
    TextRange,
    TextRangeView(TextUnit),
    TextRangeOrNone,
    TextRangeList,
    Encoding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextUnit {
    Bytes,
    Scalars,
    Graphemes,
}

impl std::fmt::Display for ValueType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scalar(ty) => ty.fmt(formatter),
            Self::ScalarOrNone(ty) => write!(formatter, "{ty}|none"),
            Self::OverflowResult(ty) => write!(formatter, "overflow-result of {ty}"),
            Self::DivRemResult(ty) => write!(formatter, "div-rem-result of {ty}"),
            Self::StringView(TextUnit::Bytes) => formatter.write_str("string.bytes"),
            Self::StringView(TextUnit::Scalars) => formatter.write_str("string.scalars"),
            Self::StringView(TextUnit::Graphemes) => formatter.write_str("string.graphemes"),
            Self::StringList => formatter.write_str("list of string"),
            Self::TextRange => formatter.write_str("text-range"),
            Self::TextRangeView(TextUnit::Bytes) => formatter.write_str("text-range.bytes"),
            Self::TextRangeView(TextUnit::Scalars) => formatter.write_str("text-range.scalars"),
            Self::TextRangeView(TextUnit::Graphemes) => formatter.write_str("text-range.graphemes"),
            Self::TextRangeOrNone => formatter.write_str("text-range|none"),
            Self::TextRangeList => formatter.write_str("list of text-range"),
            Self::Encoding => formatter.write_str("encoding"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArithmeticFamily {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    DivRem,
    Negate,
    ShiftLeft,
    ShiftRight,
}

impl ArithmeticFamily {
    pub(crate) const fn source_name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Subtract => "subtract",
            Self::Multiply => "multiply",
            Self::Divide => "divide",
            Self::Remainder => "remainder",
            Self::DivRem => "div-rem",
            Self::Negate => "negate",
            Self::ShiftLeft => "shift-left",
            Self::ShiftRight => "shift-right",
        }
    }

    fn from_source_name(name: &str) -> Option<Self> {
        match name {
            "add" => Some(Self::Add),
            "subtract" => Some(Self::Subtract),
            "multiply" => Some(Self::Multiply),
            "divide" => Some(Self::Divide),
            "remainder" => Some(Self::Remainder),
            "div-rem" => Some(Self::DivRem),
            "negate" => Some(Self::Negate),
            "shift-left" => Some(Self::ShiftLeft),
            "shift-right" => Some(Self::ShiftRight),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberFamily {
    Coerce,
    Parse,
    Radix,
    Arithmetic(ArithmeticFamily),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundMethod {
    pub receiver: Span,
    pub family: MemberFamily,
    pub child: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoercionPolicy {
    Default,
    Checked,
    Wrap,
    Saturate,
}

impl CoercionPolicy {
    pub(crate) fn from_member(member: &str) -> Option<Self> {
        match member {
            "checked" => Some(Self::Checked),
            "wrap" => Some(Self::Wrap),
            "saturate" => Some(Self::Saturate),
            _ => None,
        }
    }

    fn source_name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Checked => "checked",
            Self::Wrap => "wrap",
            Self::Saturate => "saturate",
        }
    }

    fn invocation_name(self) -> &'static str {
        match self {
            Self::Default => ".coerce",
            Self::Checked => ".coerce.checked",
            Self::Wrap => ".coerce.wrap",
            Self::Saturate => ".coerce.saturate",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedBinding {
    pub name: String,
    pub span: Span,
    pub value_type: ValueType,
    pub destination_arms: Vec<ScalarType>,
    pub storage_type: Option<ScalarType>,
    pub mutable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DescriptorAlias {
    visible_from: usize,
    scope: Option<Span>,
    value_type: ScalarType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionContract {
    pub name: String,
    pub span: Span,
    pub parameters: Vec<ParameterContract>,
    pub return_type: Option<ValueType>,
    pub throws: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterContract {
    pub name: String,
    pub span: Span,
    pub value_type: Option<ValueType>,
    pub optional: bool,
    pub mutable: bool,
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

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ContextualConstant {
    Integer(BigInt),
    Float32(f32),
    Float64(f64),
}

#[derive(Clone, Debug)]
pub struct SemanticUnit {
    pub source: SourceFile,
    pub tree: SyntaxTree,
    pub namespace: String,
    prelude: bool,
    pub scopes: Vec<LexicalScope>,
    pub typed_bindings: Vec<TypedBinding>,
    /// Function contracts declared by every source unit in this unit's namespace.
    pub functions: Vec<FunctionContract>,
    function_aliases: BTreeMap<String, FunctionContract>,
    descriptor_aliases: BTreeMap<String, Vec<DescriptorAlias>>,
    pub unreachable_spans: Vec<Span>,
    pub evaluation_steps: Vec<EvaluationStep>,
}

impl SemanticUnit {
    /// Returns the compiler-resolved value type for an expression when it is statically known.
    pub(crate) fn inferred_value_type(&self, node: &SyntaxNode) -> Option<ValueType> {
        infer_value_type(self, node, &self.typed_bindings)
            .ok()
            .flatten()
    }

    fn descriptor_alias_at(&self, name: &str, position: usize) -> Option<ScalarType> {
        self.descriptor_aliases.get(name).and_then(|history| {
            history
                .iter()
                .rev()
                .find(|alias| alias.is_visible_at(self.source.id(), position))
                .map(|alias| alias.value_type)
        })
    }
}

impl DescriptorAlias {
    fn is_visible_at(&self, file: u32, position: usize) -> bool {
        self.visible_from <= position
            && self.scope.is_none_or(|scope| {
                scope.file == file && scope.start <= position && position <= scope.end
            })
    }
}

fn visible_descriptor_aliases(
    aliases: &BTreeMap<String, Vec<DescriptorAlias>>,
    file: u32,
    position: usize,
) -> BTreeMap<String, ScalarType> {
    aliases
        .iter()
        .filter_map(|(name, history)| {
            history
                .iter()
                .rev()
                .find(|alias| alias.is_visible_at(file, position))
                .map(|alias| (name.clone(), alias.value_type))
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct LexicalScope {
    pub span: Span,
    pub parent: Option<usize>,
    pub symbols: BTreeMap<String, Vec<Symbol>>,
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

fn parse_units(package: &Package) -> Result<Vec<SemanticUnit>, SemanticFailure> {
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
        validate_declared_names(source, &parsed.tree).map_err(|diagnostic| SemanticFailure {
            source: source.clone(),
            diagnostics: vec![diagnostic],
        })?;
        let namespace =
            declared_namespace(source, &parsed.tree).map_err(|diagnostic| SemanticFailure {
                source: source.clone(),
                diagnostics: vec![diagnostic],
            })?;
        if let Some(expected) = &unit.expected_namespace
            && &namespace != expected
        {
            let span = parsed
                .tree
                .root
                .children
                .iter()
                .find(|node| node.kind == SyntaxKind::NamespaceDeclaration)
                .map_or(Span::new(source.id(), 0, source.text().len()), |node| {
                    node.span
                });
            let diagnostic = Diagnostic::error(
                "S2020",
                format!(
                    "declared namespace `{namespace}` does not match `{expected}` required by its source directory"
                ),
                span,
            )
            .with_help(format!("declare `namespace {}`", expected.trim_start_matches('/')));
            return Err(SemanticFailure {
                source: source.clone(),
                diagnostics: vec![diagnostic],
            });
        }
        units.push(SemanticUnit {
            source: source.clone(),
            tree: parsed.tree,
            namespace,
            prelude: package.prelude,
            scopes: Vec::new(),
            typed_bindings: Vec::new(),
            functions: Vec::new(),
            function_aliases: BTreeMap::new(),
            descriptor_aliases: BTreeMap::new(),
            unreachable_spans: Vec::new(),
            evaluation_steps: Vec::new(),
        });
    }
    Ok(units)
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
    let mut units = parse_units(package)?;

    let mut namespaces = bootstrap_namespaces();
    for unit in &units {
        if matches!(
            unit.namespace.as_str(),
            "/core/output" | "/core/types" | "/core/errors" | "/core/collections"
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
                    unit.namespace
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
        unit.scopes = collect_lexical_scopes(unit, &namespaces, &globals)?;
    }
    let prelude_bindings = if package.prelude {
        bootstrap_prelude()
    } else {
        BTreeMap::new()
    };
    let descriptor_constructs = bootstrap_descriptor_constructs();

    let mut semantic = SemanticPackage {
        identity: package.identity.clone(),
        prelude: package.prelude,
        namespaces,
        globals,
        prelude_bindings,
        descriptor_constructs,
        units,
        bootstrap_version: BOOTSTRAP_VERSION,
    };
    validate_initializer_dependencies(&semantic)?;
    validate_references(&semantic)?;
    validate_error_clauses(&semantic)?;
    analyze_types(&mut semantic)?;
    infer_throwing_effects(&mut semantic);
    validate_constant_reassignment(&semantic)?;
    validate_global_definite_assignment(&semantic)?;
    record_binding_mutability(&mut semantic);
    validate_calls(&semantic)?;
    validate_definite_assignment(&semantic)?;
    let unreachable_units = validate_control_flow(&semantic)?;
    for (unit, unreachable_spans) in semantic.units.iter_mut().zip(unreachable_units) {
        unit.unreachable_spans = unreachable_spans;
        unit.evaluation_steps = collect_evaluation_steps(&unit.source, &unit.tree.root);
    }
    Ok(semantic)
}

fn validate_error_clauses(package: &SemanticPackage) -> Result<(), SemanticFailure> {
    fn visit(
        package: &SemanticPackage,
        unit: &SemanticUnit,
        node: &SyntaxNode,
        in_catch: bool,
    ) -> Result<(), SemanticFailure> {
        if node.kind == SyntaxKind::ThrowStatement && node.children.is_empty() && !in_catch {
            return Err(failure(
                &unit.source,
                "T0020",
                "bare `throw` is only valid inside a catch clause",
                node.span,
            ));
        }
        if node.kind == SyntaxKind::TryStatement {
            let mut caught = BTreeSet::new();
            let mut catches_all = false;
            for clause in node
                .children
                .iter()
                .filter(|child| child.kind == SyntaxKind::CatchClause)
            {
                if let Some(alias) = clause
                    .children
                    .iter()
                    .find(|child| child.kind == SyntaxKind::CatchBinding)
                {
                    return Err(failure(
                        &unit.source,
                        "T0027",
                        "catch aliases are unavailable until error values expose source-level members",
                        alias.span,
                    ));
                }
                let Some(descriptor) = clause
                    .children
                    .first()
                    .filter(|child| child.kind == SyntaxKind::Name)
                else {
                    if catches_all {
                        return Err(failure(
                            &unit.source,
                            "T0022",
                            "catch-all clause is unreachable",
                            clause.span,
                        ));
                    }
                    catches_all = true;
                    continue;
                };
                let name = node_text(&unit.source, descriptor);
                let symbol = package.resolve_name_at(unit, descriptor.span.start, name);
                let valid = symbol.is_some_and(|symbol| {
                    symbol.kind == SymbolKind::ErrorObject
                        || (symbol.kind == SymbolKind::Interface
                            && symbol.identity == "/core/errors::error")
                });
                if !valid {
                    return Err(failure(
                        &unit.source,
                        "T0021",
                        format!("`{name}` is not an error descriptor"),
                        descriptor.span,
                    ));
                }
                let identity = &symbol.expect("validated error symbol").identity;
                if catches_all || !caught.insert(identity.clone()) {
                    return Err(failure(
                        &unit.source,
                        "T0022",
                        format!("catch clause for `{name}` is unreachable"),
                        clause.span,
                    ));
                }
                catches_all = identity == "/core/errors::error";
            }
        }
        for child in &node.children {
            let child_in_catch = in_catch || node.kind == SyntaxKind::CatchClause;
            visit(package, unit, child, child_in_catch)?;
        }
        Ok(())
    }

    for unit in &package.units {
        visit(package, unit, &unit.tree.root, false)?;
    }
    Ok(())
}

fn populate_namespace_function_contracts(package: &mut SemanticPackage) {
    let namespaces = package
        .units
        .iter()
        .map(|unit| unit.namespace.clone())
        .collect::<Vec<_>>();
    let functions = package
        .units
        .iter()
        .map(|unit| unit.functions.clone())
        .collect::<Vec<_>>();
    for (unit, namespace) in package.units.iter_mut().zip(&namespaces) {
        unit.functions = namespaces
            .iter()
            .zip(&functions)
            .filter(|(candidate, _)| *candidate == namespace)
            .flat_map(|(_, functions)| functions.iter().cloned())
            .collect();
    }
}

fn populate_function_aliases(package: &mut SemanticPackage) {
    let contracts = package
        .units
        .iter()
        .flat_map(|unit| unit.functions.iter())
        .map(|contract| {
            (
                (contract.span.file, contract.span.start, contract.span.end),
                contract.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for unit in &mut package.units {
        unit.function_aliases = package
            .namespaces
            .get(&unit.namespace)
            .into_iter()
            .flat_map(|namespace| &namespace.symbols)
            .filter_map(|(visible_name, symbol)| {
                let span = symbol.declaration_span?;
                (symbol.kind == SymbolKind::Function)
                    .then(|| contracts.get(&(span.file, span.start, span.end)))
                    .flatten()
                    .cloned()
                    .map(|contract| (visible_name.clone(), contract))
            })
            .collect();
    }
}

impl SemanticPackage {
    #[must_use]
    pub fn symbol(&self, namespace: &str, name: &str) -> Option<&Symbol> {
        self.namespaces.get(namespace)?.symbols.get(name)
    }

    #[must_use]
    pub fn resolve_name(&self, namespace: &str, name: &str) -> Option<&Symbol> {
        namespace_chain(namespace)
            .find_map(|path| {
                self.symbol(&path, name).filter(|symbol| {
                    visible_from(symbol, namespace)
                        && (symbol.kind != SymbolKind::Binding
                            || symbol.constant
                            || symbol.global
                            || symbol.namespace == namespace)
                })
            })
            .or_else(|| {
                self.globals
                    .get(name)
                    .filter(|symbol| visible_from(symbol, namespace))
            })
            .or_else(|| self.prelude_bindings.get(name))
    }

    #[must_use]
    pub fn resolve_name_at<'a>(
        &'a self,
        unit: &'a SemanticUnit,
        offset: usize,
        name: &str,
    ) -> Option<&'a Symbol> {
        let mut scopes = lexical_scope_chain(unit, offset).peekable();
        let inside_lexical_scope = scopes.peek().is_some();
        scopes
            .find_map(|scope| {
                scope.symbols.get(name)?.iter().rev().find(|symbol| {
                    symbol
                        .declaration_span
                        .is_none_or(|span| span.end <= offset)
                })
            })
            .or_else(|| {
                self.resolve_name(&unit.namespace, name)
                    .filter(|symbol| !inside_lexical_scope || symbol.available_in_function_body())
            })
    }

    #[must_use]
    pub fn is_lexical_replacement(&self, unit: &SemanticUnit, span: Span, name: &str) -> bool {
        let Some(current) = unit
            .typed_bindings
            .iter()
            .find(|binding| binding.name == name && binding.span == span)
        else {
            return false;
        };
        let current_scope = lexical_scope_index_at(unit, current.span.start);
        lexical_scope_chain(unit, span.start).any(|scope| {
            scope.symbols.get(name).is_some_and(|symbols| {
                symbols
                    .iter()
                    .any(|symbol| symbol.declaration_span == Some(span))
                    && symbols.iter().any(|symbol| {
                        symbol.declaration_span.is_some_and(|prior| {
                            prior.start < span.start
                                && lexical_scope_index_at(unit, prior.start) == current_scope
                        })
                    })
            })
        })
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
        .map(|child| {
            let component = node_text(source, child);
            validate_namespace_segment(component, child.span)?;
            Ok(component)
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    normalize_declared_path(&components).ok_or_else(|| {
        Diagnostic::error(
            "S2003",
            "namespace declaration requires an unanchored path",
            declarations[0].span,
        )
    })
}

fn validate_namespace_segment(component: &str, span: Span) -> Result<(), Diagnostic> {
    fn valid(component: &str) -> bool {
        let mut bytes = component.bytes();
        let Some(first) = bytes.next() else {
            return false;
        };
        if !first.is_ascii_lowercase() {
            return false;
        }
        let mut previous_hyphen = false;
        for byte in bytes {
            if byte == b'-' {
                if previous_hyphen {
                    return false;
                }
                previous_hyphen = true;
            } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
                previous_hyphen = false;
            } else {
                return false;
            }
        }
        !previous_hyphen
    }

    if !valid(component) {
        let lowercase = component.to_ascii_lowercase();
        let mut diagnostic = Diagnostic::error(
            "S2018",
            format!(
                "invalid namespace segment `{component}`; segments must match `[a-z]([a-z0-9]|-[a-z0-9])*`"
            ),
            span,
        );
        if lowercase != component && valid(&lowercase) {
            diagnostic = diagnostic.with_help(format!("use `{lowercase}`"));
        }
        return Err(diagnostic);
    }
    if is_reserved_namespace_segment(component) {
        return Err(Diagnostic::error(
            "S2019",
            format!("namespace segment `{component}` is reserved"),
            span,
        )
        .with_help(format!(
            "choose a different name, such as `{component}-app`"
        )));
    }
    Ok(())
}

fn is_reserved_namespace_segment(component: &str) -> bool {
    matches!(component, "con" | "prn" | "aux" | "nul")
        || component
            .strip_prefix("com")
            .or_else(|| component.strip_prefix("lpt"))
            .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
}

fn validate_declared_names(source: &SourceFile, tree: &SyntaxTree) -> Result<(), Diagnostic> {
    fn visit(source: &SourceFile, node: &SyntaxNode) -> Result<(), Diagnostic> {
        let declared_children = matches!(
            node.kind,
            SyntaxKind::Binding
                | SyntaxKind::FunctionDeclaration
                | SyntaxKind::Parameter
                | SyntaxKind::ForTarget
                | SyntaxKind::ImportAlias
        );
        if declared_children {
            for child in &node.children {
                if child.kind == SyntaxKind::Name {
                    let authored = node_text(source, child);
                    if authored.bytes().any(|byte| byte.is_ascii_uppercase()) {
                        let replacement = authored.to_ascii_lowercase();
                        return Err(Diagnostic::error(
                            "S2018",
                            format!("declared name `{authored}` must be lowercase"),
                            child.span,
                        )
                        .with_help(format!("use `{replacement}`")));
                    }
                }
            }
        }
        for child in &node.children {
            visit(source, child)?;
        }
        Ok(())
    }
    visit(source, &tree.root)
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
        ) && declaration_from_syntax(unit, child).is_some_and(|declaration| declaration.global)
        {
            collect_declaration(unit, child, namespaces, globals)?;
        }
        collect_nested_declarations(unit, child, namespaces, globals)?;
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct Declaration {
    name: String,
    visibility: Visibility,
    explicit_visibility: bool,
    global: bool,
    constant: bool,
    kind: SymbolKind,
}

fn declaration_from_syntax(unit: &SemanticUnit, node: &SyntaxNode) -> Option<Declaration> {
    let name_node = node
        .children
        .iter()
        .find(|child| child.kind == SyntaxKind::Name)?;
    let name = node_text(&unit.source, name_node).to_owned();
    let visibility_node = node
        .children
        .iter()
        .find(|child| child.kind == SyntaxKind::Visibility);
    let visibility = visibility_node
        .map(|child| node_text(&unit.source, child))
        .map_or(Visibility::Public, |visibility| match visibility {
            "private" => Visibility::Private,
            "protected" => Visibility::Protected,
            _ => Visibility::Public,
        });
    let qualifier = |expected| {
        node.children.iter().any(|child| {
            child.kind == SyntaxKind::DeclarationQualifier
                && node_text(&unit.source, child) == expected
        })
    };
    let kind = if node.kind == SyntaxKind::FunctionDeclaration {
        SymbolKind::Function
    } else {
        SymbolKind::Binding
    };
    Some(Declaration {
        name,
        visibility,
        explicit_visibility: visibility_node.is_some(),
        global: qualifier("global"),
        constant: qualifier("constant"),
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
    if node.kind == SyntaxKind::Assignment && globals.contains_key(&declaration.name) {
        return Ok(());
    }
    if declaration.kind == SymbolKind::Binding
        && !declaration.constant
        && !declaration.global
        && declaration.explicit_visibility
        && declaration.visibility == Visibility::Public
    {
        return Err(failure(
            &unit.source,
            "S2025",
            format!("namespace variable `{}` cannot be public", declaration.name),
            node.span,
        ));
    }
    let identity = if declaration.global {
        format!("global::{}", declaration.name)
    } else {
        format!("{}::{}", unit.namespace, declaration.name)
    };
    let symbol = Symbol {
        identity,
        name: declaration.name.clone(),
        namespace: unit.namespace.clone(),
        visibility: declaration.visibility,
        global: declaration.global,
        constant: declaration.constant,
        kind: declaration.kind,
        declaration_span: Some(node.span),
    };
    if declaration.global {
        globals.insert(declaration.name, symbol);
        return Ok(());
    }
    let table = &mut namespaces
        .get_mut(&unit.namespace)
        .expect("every source-unit namespace is assembled before declarations")
        .symbols;
    if node.kind == SyntaxKind::Assignment
        && table.get(&declaration.name).is_some_and(|existing| {
            existing
                .declaration_span
                .is_some_and(|span| span.file == node.span.file)
        })
    {
        return Ok(());
    }
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
    let imports = node
        .children
        .iter()
        .filter(|child| child.kind == SyntaxKind::ObjectImport);
    let mut result = Vec::new();
    for import_node in imports {
        let imported_node = import_node
            .children
            .iter()
            .find(|child| child.kind == SyntaxKind::Name)
            .ok_or_else(|| {
                failure(
                    &unit.source,
                    "S2008",
                    "import has no name",
                    import_node.span,
                )
            })?;
        let imported = node_text(&unit.source, imported_node);
        let alias = import_node
            .children
            .iter()
            .find(|child| child.kind == SyntaxKind::ImportAlias)
            .and_then(|alias| {
                alias
                    .children
                    .iter()
                    .find(|child| child.kind == SyntaxKind::Name)
            })
            .map_or(imported, |alias| node_text(&unit.source, alias));
        result.push(Import {
            source: unit.source.clone(),
            namespace: unit.namespace.clone(),
            target: target.clone(),
            object: imported.to_owned(),
            alias: alias.to_owned(),
            span: import_node.span,
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
        .and_then(|namespace| namespace.symbols.get(&import.object))
        .ok_or_else(|| {
            failure(
                &import.source,
                "S2009",
                format!("unresolved name `{}` in `{}`", import.object, import.target),
                import.span,
            )
        })?;
    if !visible_from(export, &import.namespace) {
        return Err(failure(
            &import.source,
            "S2010",
            format!("name `{}` is inaccessible", import.object),
            import.span,
        ));
    }
    if !export.available_in_function_body() {
        return Err(namespace_variable_import_failure(
            &import.source,
            &import.object,
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
        if let Some(existing) = destination.symbols.get(&import.alias) {
            if existing.identity == export.identity {
                continue;
            }
            return Err(failure(
                &import.source,
                "S2011",
                format!("import `{}` collides; use an alias", import.alias),
                import.span,
            ));
        }
        destination.symbols.insert(import.alias, export);
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
                    .resolve_name_at(unit, node.span.start, name)
                    .is_none()
                    && !package.descriptor_constructs.contains_key(name)
                {
                    if namespace_chain(&unit.namespace)
                        .filter_map(|path| package.namespaces.get(&path))
                        .filter_map(|namespace| namespace.symbols.get(name))
                        .chain(package.globals.get(name))
                        .any(|symbol| {
                            symbol.kind == SymbolKind::Binding
                                && !symbol.available_in_function_body()
                        })
                    {
                        return Err(namespace_variable_reference_failure(
                            &unit.source,
                            name,
                            node.span,
                        ));
                    }
                    return Err(failure(
                        &unit.source,
                        "S2013",
                        format!("unresolved name `{name}`"),
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
                    if !declaration_name_skipped && child.kind == SyntaxKind::Name {
                        declaration_name_skipped = true;
                        continue;
                    }
                    visit(package, unit, child)?;
                }
            }
            SyntaxKind::CatchClause => {
                if let Some(descriptor) = node.children.first() {
                    visit(package, unit, descriptor)?;
                }
                if let Some(block) = node.children.last()
                    && block.kind == SyntaxKind::Block
                {
                    visit(package, unit, block)?;
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
fn namespace_variable_reference_failure(
    source: &SourceFile,
    name: &str,
    span: Span,
) -> SemanticFailure {
    SemanticFailure {
        source: source.clone(),
        diagnostics: vec![
            Diagnostic::error(
                "S2026",
                format!("namespace variable `{name}` cannot cross a function boundary"),
                span,
            )
            .with_help(format!(
                "pass `{name}` as a parameter or return it from a function"
            )),
        ],
    }
}

fn namespace_variable_import_failure(
    source: &SourceFile,
    name: &str,
    span: Span,
) -> SemanticFailure {
    SemanticFailure {
        source: source.clone(),
        diagnostics: vec![
            Diagnostic::error(
                "S2026",
                format!("namespace variable `{name}` cannot be imported outside its namespace"),
                span,
            )
            .with_help(format!(
                "import a function that reads `{name}` and returns its value instead"
            )),
        ],
    }
}

fn binding_initializer(node: &SyntaxNode) -> Option<&SyntaxNode> {
    let name_index = node
        .children
        .iter()
        .position(|child| child.kind == SyntaxKind::Name)?;
    node.children
        .iter()
        .enumerate()
        .rev()
        .find(|(index, child)| {
            *index != name_index
                && !matches!(
                    child.kind,
                    SyntaxKind::TypeExpression
                        | SyntaxKind::Visibility
                        | SyntaxKind::DeclarationQualifier
                )
        })
        .map(|(_, child)| child)
}

#[expect(
    clippy::too_many_lines,
    reason = "the dependency graph construction and its diagnostics are one ordered validation pass"
)]
fn validate_initializer_dependencies(package: &SemanticPackage) -> Result<(), SemanticFailure> {
    type Key = (u32, usize, usize);

    fn key(span: Span) -> Key {
        (span.file, span.start, span.end)
    }

    fn collect_reads(
        package: &SemanticPackage,
        unit: &SemanticUnit,
        node: &SyntaxNode,
        reads: &mut Vec<(Key, Span)>,
        functions: &mut BTreeSet<Key>,
    ) {
        if node.kind == SyntaxKind::Name {
            if let Some(symbol) =
                package.resolve_name_at(unit, node.span.start, node_text(&unit.source, node))
                && let Some(span) = symbol.declaration_span
            {
                if symbol.kind == SymbolKind::Binding && !symbol.global {
                    reads.push((key(span), node.span));
                } else if symbol.kind == SymbolKind::Function && functions.insert(key(span)) {
                    for owner in &package.units {
                        if let Some(function) = find_node_by_span(&owner.tree.root, span) {
                            collect_reads(package, owner, function, reads, functions);
                            break;
                        }
                    }
                }
            }
            return;
        }
        if matches!(
            node.kind,
            SyntaxKind::NamespaceDeclaration
                | SyntaxKind::ImportDeclaration
                | SyntaxKind::Parameter
                | SyntaxKind::ForTarget
                | SyntaxKind::TypeExpression
                | SyntaxKind::UnionType
                | SyntaxKind::PrefixType
                | SyntaxKind::AppliedType
                | SyntaxKind::FunctionType
        ) {
            return;
        }
        for child in &node.children {
            collect_reads(package, unit, child, reads, functions);
        }
    }
    fn unresolved_name_span(
        package: &SemanticPackage,
        unit: &SemanticUnit,
        node: &SyntaxNode,
        name: &str,
    ) -> Option<Span> {
        if node.kind == SyntaxKind::Name
            && node_text(&unit.source, node) == name
            && package
                .resolve_name_at(unit, node.span.start, name)
                .is_none()
        {
            return Some(node.span);
        }
        node.children
            .iter()
            .find_map(|child| unresolved_name_span(package, unit, child, name))
    }
    fn validate_self_references(
        package: &SemanticPackage,
        unit: &SemanticUnit,
        node: &SyntaxNode,
    ) -> Result<(), SemanticFailure> {
        if node.kind == SyntaxKind::Binding
            && let Some(initializer) = binding_initializer(node)
        {
            let declaration =
                declaration_from_syntax(unit, node).expect("ordinary binding has a name");
            let mut reads = Vec::new();
            collect_reads(package, unit, initializer, &mut reads, &mut BTreeSet::new());
            let direct_unresolved_self =
                unresolved_name_span(package, unit, initializer, &declaration.name);
            if let Some(span) = reads
                .iter()
                .find(|(dependency, _)| *dependency == key(node.span))
                .map(|(_, span)| *span)
                .or(direct_unresolved_self)
            {
                return Err(failure(
                    &unit.source,
                    "S2023",
                    format!(
                        "binding `{}` cannot reference itself in its initializer",
                        declaration.name
                    ),
                    span,
                ));
            }
        }
        for child in &node.children {
            validate_self_references(package, unit, child)?;
        }
        Ok(())
    }

    fn find_cycle(
        current: Key,
        edges: &BTreeMap<Key, Vec<(Key, Span)>>,
        path: &mut BTreeSet<Key>,
    ) -> Option<Span> {
        if !path.insert(current) {
            return None;
        }
        for &(dependency, span) in edges.get(&current).into_iter().flatten() {
            if path.contains(&dependency) {
                return Some(span);
            }
            if let Some(span) = find_cycle(dependency, edges, path) {
                return Some(span);
            }
        }
        path.remove(&current);
        None
    }
    for unit in &package.units {
        validate_self_references(package, unit, &unit.tree.root)?;
    }

    let mut edges = BTreeMap::<Key, Vec<(Key, Span)>>::new();
    for unit in &package.units {
        for node in &unit.tree.root.children {
            if node.kind != SyntaxKind::Binding {
                continue;
            }
            let Some(declaration) = declaration_from_syntax(unit, node) else {
                continue;
            };
            let Some(initializer) = binding_initializer(node) else {
                continue;
            };
            let mut reads = Vec::new();
            collect_reads(package, unit, initializer, &mut reads, &mut BTreeSet::new());
            if reads
                .iter()
                .any(|(dependency, _)| *dependency == key(node.span))
            {
                let span = reads
                    .iter()
                    .find(|(dependency, _)| *dependency == key(node.span))
                    .expect("checked self-reference")
                    .1;
                return Err(failure(
                    &unit.source,
                    "S2023",
                    format!(
                        "binding `{}` cannot reference itself in its initializer",
                        declaration.name
                    ),
                    span,
                ));
            }
            if !declaration.global {
                edges.entry(key(node.span)).or_default().extend(reads);
            }
        }
    }
    for unit in &package.units {
        for node in &unit.tree.root.children {
            if node.kind != SyntaxKind::Assignment {
                continue;
            }
            let Some(declaration) = declaration_from_syntax(unit, node) else {
                continue;
            };
            let name = node
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Name)
                .expect("ordinary assignment has a name");
            if package
                .globals
                .get(&declaration.name)
                .is_some_and(|global| global.namespace == unit.namespace)
                && !declaration.global
            {
                return Err(SemanticFailure {
                    source: unit.source.clone(),
                    diagnostics: vec![
                        Diagnostic::error(
                            "S2021",
                            format!(
                                "plain namespace assignment cannot replace program-global binding `{}`",
                                declaration.name
                            ),
                            name.span,
                        )
                        .with_help(
                            "pass changing values through parameters and returns instead",
                        ),
                    ],
                });
            }
            let Some(target) = package.resolve_name_at(unit, name.span.start, &declaration.name)
            else {
                continue;
            };
            let Some(initializer) = binding_initializer(node) else {
                continue;
            };
            let Some(owner) = target.declaration_span else {
                continue;
            };
            let mut reads = Vec::new();
            collect_reads(package, unit, initializer, &mut reads, &mut BTreeSet::new());
            reads.retain(|(dependency, _)| *dependency != key(owner));
            edges.entry(key(owner)).or_default().extend(reads);
        }
    }
    for &start in edges.keys() {
        if let Some(span) = find_cycle(start, &edges, &mut BTreeSet::new()) {
            let source = package
                .units
                .iter()
                .find(|unit| unit.source.id() == span.file)
                .expect("dependency span belongs to a semantic unit");
            return Err(failure(
                &source.source,
                "S2024",
                "namespace binding initialization has a dependency cycle",
                span,
            ));
        }
    }
    Ok(())
}

fn find_node_by_span(node: &SyntaxNode, span: Span) -> Option<&SyntaxNode> {
    (node.span == span).then_some(node).or_else(|| {
        node.children
            .iter()
            .find_map(|child| find_node_by_span(child, span))
    })
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

fn validate_calls(package: &SemanticPackage) -> Result<(), SemanticFailure> {
    let contracts = package
        .units
        .iter()
        .flat_map(|unit| &unit.functions)
        .map(|contract| {
            (
                (contract.span.file, contract.span.start, contract.span.end),
                contract,
            )
        })
        .collect();
    for unit in &package.units {
        let bindings = call_site_bindings(unit, None);
        validate_call_nodes(package, unit, &unit.tree.root, &contracts, None, &bindings)?;
    }
    Ok(())
}

fn validate_string_member_expression(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    bindings: &[TypedBinding],
) -> Result<(), SemanticFailure> {
    let member = (node.kind == SyntaxKind::MemberExpression)
        .then(|| node.children.get(1))
        .flatten()
        .map(|member| node_text(&unit.source, member));
    let call_member = (node.kind == SyntaxKind::CallExpression)
        .then(|| node.children.first())
        .flatten()
        .filter(|callee| callee.kind == SyntaxKind::MemberExpression)
        .and_then(|callee| callee.children.get(1))
        .map(|member| node_text(&unit.source, member));
    if member == Some("length") || matches!(call_member, Some("concat" | "join")) {
        infer_value_type(unit, node, bindings)?;
    }
    Ok(())
}

#[expect(
    clippy::too_many_lines,
    reason = "call validation remains one traversal so every call form shares lexical scope and contracts"
)]
fn validate_call_nodes<'a>(
    package: &SemanticPackage,
    unit: &'a SemanticUnit,
    node: &SyntaxNode,
    contracts: &BTreeMap<(u32, usize, usize), &FunctionContract>,
    active_function: Option<&'a FunctionContract>,
    scoped_bindings: &[TypedBinding],
) -> Result<(), SemanticFailure> {
    let entered_function = (node.kind == SyntaxKind::FunctionDeclaration)
        .then(|| {
            unit.functions
                .iter()
                .find(|contract| contract.span == node.span)
        })
        .flatten();
    let active_function = entered_function.or(active_function);
    let function_bindings =
        entered_function.map(|contract| call_site_bindings(unit, Some(contract)));
    let scoped_bindings = function_bindings.as_deref().unwrap_or(scoped_bindings);

    validate_resolved_assignment(package, unit, node, contracts)?;
    validate_integer_coercion_call(unit, node, scoped_bindings)?;
    if node.kind == SyntaxKind::CallExpression
        && let Some(arguments) = node.children.get(1)
    {
        for argument in &arguments.children {
            let value = argument.children.last().unwrap_or(argument);
            infer_value_type(unit, value, scoped_bindings)?;
        }
    }
    if node.kind == SyntaxKind::CallExpression
        && let [callee, arguments] = node.children.as_slice()
        && callee.kind == SyntaxKind::Name
        && package
            .resolve_name_at(unit, callee.span.start, node_text(&unit.source, callee))
            .is_some_and(|symbol| symbol.identity == "/core/output::print")
    {
        for argument in &arguments.children {
            let value = argument.children.last().unwrap_or(argument);
            let value_type = infer_value_type(unit, value, scoped_bindings)?;
            if !matches!(
                value_type,
                Some(ValueType::Scalar(
                    ScalarType::Bool
                        | ScalarType::Int
                        | ScalarType::Int8
                        | ScalarType::Int16
                        | ScalarType::Int32
                        | ScalarType::Int64
                        | ScalarType::Int128
                        | ScalarType::Uint8
                        | ScalarType::Uint16
                        | ScalarType::Uint32
                        | ScalarType::Uint64
                        | ScalarType::Uint128
                        | ScalarType::Float32
                        | ScalarType::Float64
                        | ScalarType::String
                        | ScalarType::None
                ))
            ) {
                return Err(failure(
                    &unit.source,
                    "T0035",
                    format!(
                        "`print` requires a text-displayable scalar value, found {}",
                        value_type.map_or_else(|| "unknown".to_owned(), |ty| ty.to_string())
                    ),
                    value.span,
                ));
            }
        }
    }
    if node.kind == SyntaxKind::CallExpression
        && let [callee, arguments] = node.children.as_slice()
        && callee.kind == SyntaxKind::Name
        && let Some(symbol) =
            package.resolve_name_at(unit, callee.span.start, node_text(&unit.source, callee))
        && symbol.kind == SymbolKind::Function
        && let Some(declaration_span) = symbol.declaration_span
        && let Some(contract) = contracts.get(&(
            declaration_span.file,
            declaration_span.start,
            declaration_span.end,
        ))
    {
        validate_call_arguments(unit, arguments, contract, scoped_bindings)?;
    }
    if let [target, collection, block] = node.children.as_slice()
        && node.kind == SyntaxKind::ForStatement
        && target.kind == SyntaxKind::ForTarget
    {
        validate_call_nodes(
            package,
            unit,
            collection,
            contracts,
            active_function,
            scoped_bindings,
        )?;
        let mut loop_bindings = scoped_bindings.to_vec();
        loop_bindings.extend(target.children.iter().map(|name| TypedBinding {
            name: node_text(&unit.source, name).to_owned(),
            span: name.span,
            value_type: ValueType::Scalar(ScalarType::String),
            destination_arms: Vec::new(),
            storage_type: None,
            mutable: false,
        }));
        validate_call_nodes(
            package,
            unit,
            block,
            contracts,
            active_function,
            &loop_bindings,
        )?;
        return Ok(());
    }
    validate_string_member_expression(unit, node, scoped_bindings)?;
    validate_coercion_family_expression(unit, node)?;
    for (index, child) in node.children.iter().enumerate() {
        if node.kind == SyntaxKind::CallExpression
            && index == 0
            && let Some((source, _)) = integer_coercion_call(&unit.source, child)
        {
            validate_call_nodes(
                package,
                unit,
                source,
                contracts,
                active_function,
                scoped_bindings,
            )?;
            continue;
        }
        validate_call_nodes(
            package,
            unit,
            child,
            contracts,
            active_function,
            scoped_bindings,
        )?;
    }
    Ok(())
}

fn validate_integer_coercion_call(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    bindings: &[TypedBinding],
) -> Result<(), SemanticFailure> {
    if node.kind == SyntaxKind::CallExpression {
        infer_integer_coercion_type(unit, node, bindings)?;
    }
    Ok(())
}

fn validate_coercion_family_expression(
    unit: &SemanticUnit,
    node: &SyntaxNode,
) -> Result<(), SemanticFailure> {
    if node.kind == SyntaxKind::MemberExpression && coercion_family_receiver(unit, node) {
        return Err(failure(
            &unit.source,
            "T0018",
            "`.coerce` and its policy members are not storable values before bound methods exist",
            node.span,
        ));
    }
    Ok(())
}

fn validate_resolved_assignment(
    package: &SemanticPackage,
    unit: &SemanticUnit,
    node: &SyntaxNode,
    contracts: &BTreeMap<(u32, usize, usize), &FunctionContract>,
) -> Result<(), SemanticFailure> {
    if !matches!(node.kind, SyntaxKind::Binding | SyntaxKind::Assignment) {
        return Ok(());
    }
    let Some(name_node) = node
        .children
        .iter()
        .find(|child| child.kind == SyntaxKind::Name)
    else {
        return Ok(());
    };
    let Some(initializer) = node.children.iter().rev().find(|child| {
        child.span != name_node.span
            && !matches!(
                child.kind,
                SyntaxKind::Visibility
                    | SyntaxKind::DeclarationQualifier
                    | SyntaxKind::TypeExpression
            )
    }) else {
        return Ok(());
    };
    let Some(actual) = resolved_call_type(package, unit, initializer, contracts) else {
        return Ok(());
    };
    let name = node_text(&unit.source, name_node);
    let Some(ValueType::Scalar(expected)) = unit
        .typed_bindings
        .iter()
        .rev()
        .find(|binding| binding.name == name && binding.span.start <= node.span.start)
        .map(|binding| binding.value_type)
    else {
        return Ok(());
    };
    validate_numeric_destination(&unit.source, name, expected, actual, initializer, "T0002")
}

fn resolved_call_type(
    package: &SemanticPackage,
    unit: &SemanticUnit,
    node: &SyntaxNode,
    contracts: &BTreeMap<(u32, usize, usize), &FunctionContract>,
) -> Option<ValueType> {
    if node.kind == SyntaxKind::GroupExpression {
        return node
            .children
            .first()
            .and_then(|child| resolved_call_type(package, unit, child, contracts));
    }
    let [callee, _arguments] = node.children.as_slice() else {
        return None;
    };
    if node.kind != SyntaxKind::CallExpression || callee.kind != SyntaxKind::Name {
        return None;
    }
    let symbol =
        package.resolve_name_at(unit, callee.span.start, node_text(&unit.source, callee))?;
    let declaration = symbol.declaration_span?;
    contracts
        .get(&(declaration.file, declaration.start, declaration.end))?
        .return_type
}

fn validate_call_arguments(
    unit: &SemanticUnit,
    arguments: &SyntaxNode,
    contract: &FunctionContract,
    bindings: &[TypedBinding],
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
        let value = argument.children.last().unwrap_or(argument);
        let actual = infer_value_type(unit, value, bindings)?;
        if let (Some(expected), Some(actual)) = (parameter.value_type, actual) {
            validate_value_destination(
                &unit.source,
                &parameter.name,
                expected,
                actual,
                value,
                "T0012",
            )?;
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

fn call_site_bindings(
    unit: &SemanticUnit,
    active_function: Option<&FunctionContract>,
) -> Vec<TypedBinding> {
    let mut bindings = unit
        .typed_bindings
        .iter()
        .filter(|binding| {
            let owner = unit
                .functions
                .iter()
                .filter(|function| {
                    function.span.file == binding.span.file
                        && function.span.start <= binding.span.start
                        && binding.span.end <= function.span.end
                })
                .min_by_key(|function| function.span.end - function.span.start);
            owner
                .is_none_or(|owner| active_function.is_some_and(|active| active.span == owner.span))
        })
        .cloned()
        .collect::<Vec<_>>();
    if let Some(function) = active_function {
        bindings.extend(function.parameters.iter().filter_map(|parameter| {
            parameter.value_type.map(|value_type| TypedBinding {
                name: parameter.name.clone(),
                span: parameter.span,
                value_type,
                destination_arms: Vec::new(),
                storage_type: None,
                mutable: false,
            })
        }));
    }
    bindings
}

fn descriptor_construct_alias_history(
    package: &SemanticPackage,
    unit: &SemanticUnit,
) -> BTreeMap<String, Vec<DescriptorAlias>> {
    let mut aliases = package
        .descriptor_constructs
        .iter()
        .filter_map(|(name, symbol)| Some((name.clone(), symbol.descriptor_type()?)))
        .collect::<BTreeMap<_, _>>();
    if let Some(namespace) = package.namespaces.get(&unit.namespace) {
        aliases.extend(
            namespace
                .symbols
                .iter()
                .filter_map(|(name, symbol)| Some((name.clone(), symbol.descriptor_type()?))),
        );
    }
    aliases
        .into_iter()
        .map(|(name, value_type)| {
            (
                name,
                vec![DescriptorAlias {
                    visible_from: 0,
                    scope: None,
                    value_type,
                }],
            )
        })
        .collect()
}

fn analyze_types(package: &mut SemanticPackage) -> Result<(), SemanticFailure> {
    for index in 0..package.units.len() {
        let unit = &package.units[index];
        let mut alias_history = descriptor_construct_alias_history(package, unit);
        let mut functions = Vec::new();
        collect_type_declarations(
            unit,
            &unit.tree.root,
            &mut alias_history,
            &mut functions,
            None,
        )?;
        package.units[index].descriptor_aliases = alias_history;
        package.units[index].functions = functions;
    }
    populate_namespace_function_contracts(package);
    populate_function_aliases(package);
    validate_descriptor_value_uses(package)?;

    for index in 0..package.units.len() {
        let unit = &package.units[index];
        let mut visible_bindings = Vec::new();
        let mut bindings = Vec::new();
        collect_typed_bindings(unit, &unit.tree.root, &mut visible_bindings, &mut bindings)?;
        package.units[index].typed_bindings = bindings;
    }
    Ok(())
}
fn validate_descriptor_value_uses(package: &SemanticPackage) -> Result<(), SemanticFailure> {
    for unit in &package.units {
        validate_descriptor_value_node(package, unit, &unit.tree.root, false)?;
    }
    Ok(())
}

fn validate_descriptor_value_node(
    package: &SemanticPackage,
    unit: &SemanticUnit,
    node: &SyntaxNode,
    descriptor_context: bool,
) -> Result<(), SemanticFailure> {
    if node.kind == SyntaxKind::TypeMembershipExpression
        && let Some(descriptor) = node.children.get(1)
        && descriptor_expression_type(package, unit, descriptor).is_none()
        && descriptor_expression_category(package, unit, descriptor).is_none()
    {
        return Err(failure(
            &unit.source,
            "T0001",
            format!(
                "`{}` does not resolve to a type descriptor",
                node_text(&unit.source, descriptor).trim()
            ),
            descriptor.span,
        ));
    }
    if !descriptor_context
        && node.kind == SyntaxKind::Name
        && (descriptor_expression_type(package, unit, node).is_some()
            || descriptor_expression_category(package, unit, node).is_some())
    {
        return Err(failure(
            &unit.source,
            "T0019",
            format!(
                "type descriptor `{}` is a compile-time construct and cannot be used as a runtime value",
                node_text(&unit.source, node).trim_start_matches('.')
            ),
            node.span,
        ));
    }

    for (index, child) in node.children.iter().enumerate() {
        let child_is_descriptor_context = descriptor_context
            || node.kind == SyntaxKind::TypeExpression
            || node.kind == SyntaxKind::ImportDeclaration
            || (node.kind == SyntaxKind::TypeMembershipExpression && index == 1)
            || (node.kind == SyntaxKind::MemberExpression && index == 1)
            || (matches!(node.kind, SyntaxKind::Binding | SyntaxKind::Assignment) && index == 0)
            || (node.kind == SyntaxKind::BinaryExpression
                && node.children.len() == 2
                && node_text(&unit.source, node)[node.children[0].span.end - node.span.start
                    ..node.children[1].span.start - node.span.start]
                    .trim()
                    == "is")
            || (node.kind == SyntaxKind::BinaryExpression
                && node.children.len() == 2
                && matches!(
                    node_text(&unit.source, node)[node.children[0].span.end - node.span.start
                        ..node.children[1].span.start - node.span.start]
                        .trim(),
                    "==" | "!="
                )
                && node_text(&unit.source, child).trim() == "none")
            || (node.kind == SyntaxKind::CallExpression
                && index == 1
                && node.children.first().is_some_and(|callee| {
                    coercion_family_receiver(unit, callee)
                        || obsolete_integer_coercion_member(unit, callee).is_some()
                }));
        validate_descriptor_value_node(package, unit, child, child_is_descriptor_context)?;
    }
    Ok(())
}

pub(crate) fn descriptor_expression_type(
    package: &SemanticPackage,
    unit: &SemanticUnit,
    node: &SyntaxNode,
) -> Option<ScalarType> {
    let name = node_text(&unit.source, node).trim();
    match node.kind {
        SyntaxKind::Name | SyntaxKind::TypeExpression => unit
            .descriptor_alias_at(name, node.span.start)
            .or_else(|| package.descriptor_constructs.get(name)?.descriptor_type())
            .or_else(|| {
                node.children
                    .first()
                    .and_then(|child| descriptor_expression_type(package, unit, child))
            }),
        _ => None,
    }
}

pub(crate) fn descriptor_expression_category(
    package: &SemanticPackage,
    unit: &SemanticUnit,
    node: &SyntaxNode,
) -> Option<TypeCategory> {
    let name = node_text(&unit.source, node).trim();
    match node.kind {
        SyntaxKind::Name | SyntaxKind::TypeExpression => package
            .resolve_name_at(unit, node.span.start, name)
            .and_then(Symbol::descriptor_category)
            .or_else(|| {
                node.children
                    .first()
                    .and_then(|child| descriptor_expression_category(package, unit, child))
            }),
        _ => None,
    }
}

fn collect_type_declarations(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    aliases: &mut BTreeMap<String, Vec<DescriptorAlias>>,
    functions: &mut Vec<FunctionContract>,
    scope: Option<Span>,
) -> Result<(), SemanticFailure> {
    if let Some((name, alias)) = descriptor_alias(unit, node, aliases, scope) {
        aliases.entry(name).or_default().push(alias);
    }
    if node.kind == SyntaxKind::FunctionDeclaration {
        let visible = visible_descriptor_aliases(aliases, unit.source.id(), node.span.start);
        functions.push(analyze_function_contract(unit, node, &visible)?);
    }
    let child_scope = (node.kind == SyntaxKind::FunctionDeclaration)
        .then_some(node.span)
        .or(scope);
    for child in &node.children {
        collect_type_declarations(unit, child, aliases, functions, child_scope)?;
    }
    Ok(())
}

fn descriptor_alias(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    aliases: &BTreeMap<String, Vec<DescriptorAlias>>,
    scope: Option<Span>,
) -> Option<(String, DescriptorAlias)> {
    if !matches!(node.kind, SyntaxKind::Binding | SyntaxKind::Assignment) {
        return None;
    }
    let name = node
        .children
        .iter()
        .find(|child| child.kind == SyntaxKind::Name)?;
    let initializer = node.children.last()?;
    let descriptor_name = node_text(&unit.source, initializer).trim();
    let value_type = match initializer.kind {
        SyntaxKind::Name => {
            visible_descriptor_aliases(aliases, unit.source.id(), initializer.span.start)
                .get(descriptor_name)
                .copied()
        }
        _ => None,
    }?;
    Some((
        node_text(&unit.source, name).to_owned(),
        DescriptorAlias {
            visible_from: node.span.end,
            scope,
            value_type,
        },
    ))
}

fn collect_typed_bindings(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    visible_bindings: &mut Vec<TypedBinding>,
    bindings: &mut Vec<TypedBinding>,
) -> Result<(), SemanticFailure> {
    if node.kind == SyntaxKind::FunctionDeclaration {
        let contract = unit
            .functions
            .iter()
            .find(|contract| contract.span == node.span)
            .expect("analyzed function declaration must have a semantic contract");
        let mut function_bindings = visible_bindings.clone();
        function_bindings.extend(contract.parameters.iter().filter_map(|parameter| {
            parameter.value_type.map(|value_type| TypedBinding {
                name: parameter.name.clone(),
                span: parameter.span,
                value_type,
                destination_arms: Vec::new(),
                storage_type: None,
                mutable: false,
            })
        }));
        for child in &node.children {
            collect_typed_bindings(unit, child, &mut function_bindings, bindings)?;
        }
        return Ok(());
    }
    if let [target, collection, block] = node.children.as_slice()
        && node.kind == SyntaxKind::ForStatement
        && target.kind == SyntaxKind::ForTarget
        && let Some(name) = target.children.first()
    {
        collect_typed_bindings(unit, collection, visible_bindings, bindings)?;
        let item_type = if infer_value_type(unit, collection, visible_bindings)?
            == Some(ValueType::Scalar(ScalarType::Bytes))
        {
            ValueType::Scalar(ScalarType::Uint8)
        } else {
            ValueType::Scalar(ScalarType::String)
        };
        let loop_binding = TypedBinding {
            name: node_text(&unit.source, name).to_owned(),
            span: name.span,
            value_type: item_type,
            destination_arms: Vec::new(),
            storage_type: None,
            mutable: false,
        };
        let mut loop_bindings = visible_bindings.clone();
        loop_bindings.push(loop_binding);
        collect_typed_bindings(unit, block, &mut loop_bindings, bindings)?;
        return Ok(());
    }
    if matches!(node.kind, SyntaxKind::Binding | SyntaxKind::Assignment) {
        let prior_len = visible_bindings.len();
        analyze_binding_node(unit, node, visible_bindings)?;
        bindings.extend_from_slice(&visible_bindings[prior_len..]);
    }
    for child in &node.children {
        collect_typed_bindings(unit, child, visible_bindings, bindings)?;
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
        .map(|type_node| declared_value_type(unit, type_node, aliases))
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
                .map(|type_node| declared_value_type(unit, type_node, aliases))
                .transpose()?;
            let default = parameter.children.iter().find(|child| {
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
            if let (Some(expected), Some(default)) = (value_type, default) {
                let actual =
                    infer_value_type(unit, default, &unit.typed_bindings)?.ok_or_else(|| {
                        failure(
                            &unit.source,
                            "T0006",
                            "parameter default has no value",
                            default.span,
                        )
                    })?;
                validate_value_destination(
                    &unit.source,
                    node_text(&unit.source, parameter_name),
                    expected,
                    actual,
                    default,
                    "T0006",
                )?;
            }
            parameters.push(ParameterContract {
                name: node_text(&unit.source, parameter_name).to_owned(),
                span: parameter.span,
                value_type,
                optional,
                mutable: false,
            });
        }
    }
    let throws = node.children.iter().any(|child| {
        child.kind == SyntaxKind::DeclarationQualifier && node_text(&unit.source, child) == "throws"
    });
    Ok(FunctionContract {
        name: node_text(&unit.source, name_node).to_owned(),
        span: node.span,
        parameters,
        return_type,
        throws,
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "the fixed-point call graph and its local traversals form one auditable effect analysis"
)]
fn infer_throwing_effects(package: &mut SemanticPackage) {
    type FunctionKey = (u32, usize, usize);

    fn key(span: Span) -> FunctionKey {
        (span.file, span.start, span.end)
    }

    fn direct_errors(
        package: &SemanticPackage,
        unit: &SemanticUnit,
        node: &SyntaxNode,
    ) -> BTreeSet<String> {
        if node.kind == SyntaxKind::FunctionDeclaration {
            return BTreeSet::new();
        }
        if node.kind == SyntaxKind::ThrowStatement {
            return node
                .children
                .first()
                .and_then(|error| {
                    package.resolve_name_at(unit, error.span.start, node_text(&unit.source, error))
                })
                .map(|symbol| symbol.identity.clone())
                .into_iter()
                .collect();
        }
        if node.kind == SyntaxKind::TryStatement {
            let mut errors = node
                .children
                .first()
                .map_or_else(BTreeSet::new, |block| direct_errors(package, unit, block));
            let mut clauses_finished = false;
            for child in node.children.iter().skip(1) {
                if child.kind == SyntaxKind::CatchClause {
                    let descriptor = child
                        .children
                        .first()
                        .filter(|candidate| candidate.kind == SyntaxKind::Name);
                    if let Some(descriptor) = descriptor
                        && let Some(symbol) = package.resolve_name_at(
                            unit,
                            descriptor.span.start,
                            node_text(&unit.source, descriptor),
                        )
                    {
                        if symbol.identity == "/core/errors::error" {
                            errors.clear();
                        } else {
                            errors.remove(&symbol.identity);
                        }
                    } else {
                        errors.clear();
                    }
                    if let Some(block) = child.children.last() {
                        errors.extend(direct_errors(package, unit, block));
                    }
                    clauses_finished = true;
                } else if child.kind == SyntaxKind::FinallyClause {
                    if let Some(block) = child.children.last() {
                        errors.extend(direct_errors(package, unit, block));
                    }
                } else if !clauses_finished {
                    errors.extend(direct_errors(package, unit, child));
                }
            }
            return errors;
        }
        node.children
            .iter()
            .flat_map(|child| direct_errors(package, unit, child))
            .collect()
    }

    fn callees(
        package: &SemanticPackage,
        unit: &SemanticUnit,
        node: &SyntaxNode,
        output: &mut BTreeSet<FunctionKey>,
    ) {
        if node.kind == SyntaxKind::FunctionDeclaration {
            return;
        }
        if node.kind == SyntaxKind::CallExpression
            && let Some(callee) = node.children.first()
            && callee.kind == SyntaxKind::Name
            && let Some(symbol) =
                package.resolve_name_at(unit, callee.span.start, node_text(&unit.source, callee))
            && symbol.kind == SymbolKind::Function
            && let Some(span) = symbol.declaration_span
        {
            output.insert(key(span));
        }
        for child in &node.children {
            callees(package, unit, child, output);
        }
    }

    let mut effects = BTreeMap::<FunctionKey, bool>::new();
    let mut edges = BTreeMap::<FunctionKey, BTreeSet<FunctionKey>>::new();
    for unit in &package.units {
        for function in unit
            .tree
            .root
            .children
            .iter()
            .filter(|node| node.kind == SyntaxKind::FunctionDeclaration)
        {
            let function_key = key(function.span);
            let declared = unit
                .functions
                .iter()
                .find(|contract| contract.span == function.span)
                .is_some_and(|contract| contract.throws);
            effects.insert(
                function_key,
                declared
                    || function
                        .children
                        .iter()
                        .any(|child| !direct_errors(package, unit, child).is_empty()),
            );
            let mut function_callees = BTreeSet::new();
            for child in &function.children {
                callees(package, unit, child, &mut function_callees);
            }
            edges.insert(function_key, function_callees);
        }
    }

    loop {
        let mut changed = false;
        for (function, callees) in &edges {
            let inferred = effects[function]
                || callees
                    .iter()
                    .any(|callee| effects.get(callee).copied().unwrap_or(false));
            if inferred && !effects[function] {
                effects.insert(*function, true);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    for unit in &mut package.units {
        for contract in &mut unit.functions {
            contract.throws = effects.get(&key(contract.span)).copied().unwrap_or(false);
        }
    }
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

#[expect(
    clippy::too_many_lines,
    reason = "binding analysis keeps destination selection and initialization validation together"
)]
fn analyze_binding_node(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    bindings: &mut Vec<TypedBinding>,
) -> Result<(), SemanticFailure> {
    let Some(name_node) = node
        .children
        .iter()
        .find(|child| child.kind == SyntaxKind::Name)
    else {
        return Ok(());
    };
    let name = node_text(&unit.source, name_node).to_owned();
    let declared = node
        .children
        .iter()
        .find(|child| child.kind == SyntaxKind::TypeExpression);
    let initializer = node.children.iter().rev().find(|child| {
        child.span != name_node.span
            && !matches!(
                child.kind,
                SyntaxKind::Visibility
                    | SyntaxKind::DeclarationQualifier
                    | SyntaxKind::TypeExpression
            )
    });

    if node.kind == SyntaxKind::Assignment
        && declared.is_none()
        && let Some(previous) = bindings.iter().rev().find(|binding| binding.name == name)
        && let Some(initializer) = initializer
        && let Some(actual) = infer_value_type(unit, initializer, bindings)?
    {
        if previous.destination_arms.is_empty() {
            if let ValueType::Scalar(expected) = previous.value_type {
                validate_numeric_destination(
                    &unit.source,
                    &name,
                    expected,
                    actual,
                    initializer,
                    "T0002",
                )?;
            }
        } else {
            select_union_candidates(
                &unit.source,
                initializer,
                actual,
                previous.destination_arms.clone(),
            )?;
        }
        return Ok(());
    }
    let inferred = initializer
        .map(|value| infer_value_type(unit, value, bindings))
        .transpose()?
        .flatten();
    let value_type = if let Some(type_node) = declared {
        let value_type = if let (Some(inferred), Some(initializer), Ok(_)) = (
            inferred,
            initializer,
            union_destination_candidates(unit, type_node),
        ) {
            ValueType::Scalar(select_union_destination(
                unit,
                type_node,
                initializer,
                inferred,
            )?)
        } else {
            declared_value_type(
                unit,
                type_node,
                &visible_descriptor_aliases(
                    &unit.descriptor_aliases,
                    unit.source.id(),
                    type_node.span.start,
                ),
            )?
        };
        if let (Some(inferred), Some(initializer)) = (inferred, initializer) {
            validate_value_destination(
                &unit.source,
                &name,
                value_type,
                inferred,
                initializer,
                "T0002",
            )?;
        }
        value_type
    } else if let Some(inferred) = inferred {
        inferred
    } else {
        return Ok(());
    };
    let destination_arms = declared
        .and_then(|type_node| union_destination_candidates(unit, type_node).ok())
        .unwrap_or_default();
    let storage_type = (value_type == ValueType::Scalar(ScalarType::Int))
        .then(|| initializer.and_then(|value| small_int_storage(unit, value, inferred)))
        .flatten();

    bindings.push(TypedBinding {
        name,
        span: node.span,
        value_type,
        destination_arms,
        storage_type,
        mutable: false,
    });
    Ok(())
}

fn declared_value_type(
    unit: &SemanticUnit,
    type_node: &SyntaxNode,
    aliases: &BTreeMap<String, ScalarType>,
) -> Result<ValueType, SemanticFailure> {
    let type_name = node_text(&unit.source, type_node).trim();
    for (constructor, construct) in [
        (
            "overflow-result of ",
            ValueType::OverflowResult as fn(ScalarType) -> ValueType,
        ),
        (
            "div-rem-result of ",
            ValueType::DivRemResult as fn(ScalarType) -> ValueType,
        ),
    ] {
        if let Some(argument) = type_name.strip_prefix(constructor)
            && let Some(scalar) = aliases.get(argument).copied()
        {
            return Ok(construct(scalar));
        }
    }
    resolve_scalar_type(&unit.source, type_node, aliases).map(ValueType::Scalar)
}

fn union_destination_candidates(
    unit: &SemanticUnit,
    type_node: &SyntaxNode,
) -> Result<Vec<ScalarType>, SemanticFailure> {
    let Some(union) = type_node
        .children
        .first()
        .filter(|child| child.kind == SyntaxKind::UnionType)
    else {
        return Err(failure(
            &unit.source,
            "T0001",
            format!(
                "`{}` does not resolve to a scalar type descriptor",
                node_text(&unit.source, type_node).trim()
            ),
            type_node.span,
        ));
    };
    union
        .children
        .iter()
        .map(|arm| {
            let name = node_text(&unit.source, arm).trim();
            unit.descriptor_alias_at(name, arm.span.start)
                .ok_or_else(|| {
                    failure(
                        &unit.source,
                        "T0001",
                        format!("`{name}` does not resolve to a scalar type descriptor"),
                        arm.span,
                    )
                })
        })
        .collect()
}

fn select_union_destination(
    unit: &SemanticUnit,
    type_node: &SyntaxNode,
    value: &SyntaxNode,
    actual: ValueType,
) -> Result<ScalarType, SemanticFailure> {
    select_union_candidates(
        &unit.source,
        value,
        actual,
        union_destination_candidates(unit, type_node)?,
    )
}

fn select_union_candidates(
    source: &SourceFile,
    value: &SyntaxNode,
    actual: ValueType,
    candidates: Vec<ScalarType>,
) -> Result<ScalarType, SemanticFailure> {
    let is_constant = candidates
        .iter()
        .any(|candidate| contextual_constant(source, value, *candidate).is_some());
    if !is_constant
        && let ValueType::Scalar(actual) = actual
        && candidates.contains(&actual)
    {
        return Ok(actual);
    }
    let admitted = candidates
        .into_iter()
        .filter(|candidate| {
            if let Some(result) = contextual_constant(source, value, *candidate) {
                return result.is_ok();
            }
            matches!(actual, ValueType::Scalar(actual) if is_numeric(actual) && is_numeric(*candidate))
        })
        .collect::<Vec<_>>();
    match admitted.as_slice() {
        [candidate] => Ok(*candidate),
        [] => Err(failure(
            source,
            "T0002",
            "value is not admitted by any union destination arm",
            value.span,
        )),
        candidates => Err(failure(
            source,
            "T0002",
            format!(
                "numeric destination is ambiguous between {}",
                candidates
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            value.span,
        )),
    }
}

fn validate_numeric_destination(
    source: &SourceFile,
    name: &str,
    expected: ScalarType,
    actual: ValueType,
    value: &SyntaxNode,
    mismatch_code: &'static str,
) -> Result<(), SemanticFailure> {
    let ValueType::Scalar(actual) = actual else {
        return Err(failure(
            source,
            mismatch_code,
            destination_mismatch_message(mismatch_code, name, expected, actual),
            value.span,
        ));
    };
    if is_numeric(expected)
        && let Some(constant) = contextual_constant(source, value, expected)
    {
        constant?;
        return Ok(());
    }
    if actual == expected {
        return Ok(());
    }
    if is_numeric(actual) && is_numeric(expected) {
        return Ok(());
    }
    Err(failure(
        source,
        mismatch_code,
        destination_mismatch_message(mismatch_code, name, expected, ValueType::Scalar(actual)),
        value.span,
    ))
}

fn validate_value_destination(
    source: &SourceFile,
    name: &str,
    expected: ValueType,
    actual: ValueType,
    value: &SyntaxNode,
    mismatch_code: &'static str,
) -> Result<(), SemanticFailure> {
    if let ValueType::Scalar(expected) = expected {
        return validate_numeric_destination(source, name, expected, actual, value, mismatch_code);
    }
    if expected == actual {
        return Ok(());
    }
    Err(failure(
        source,
        mismatch_code,
        format!("`{name}` requires `{expected}`, found `{actual}`"),
        value.span,
    ))
}

fn destination_mismatch_message(
    code: &str,
    name: &str,
    expected: ScalarType,
    actual: ValueType,
) -> String {
    match code {
        "T0012" => {
            format!("argument for parameter `{name}` has type `{actual}`, expected `{expected}`")
        }
        "T0015" => format!("function `{name}` must return `{expected}`"),
        _ => format!("cannot assign `{actual}` to `{name}` of type `{expected}`"),
    }
}

const fn is_numeric(ty: ScalarType) -> bool {
    ty.is_integer() || matches!(ty, ScalarType::Float32 | ScalarType::Float64)
}

fn enclosed_by_guard(
    source: &SourceFile,
    current: &SyntaxNode,
    position: usize,
    name: &str,
) -> bool {
    if current.kind == SyntaxKind::IfStatement
        && let Some(condition) = current.children.first()
        && let Some(block) = current.children.iter().find(|child| {
            child.kind == SyntaxKind::Block
                && child.span.start <= position
                && position <= child.span.end
        })
    {
        let condition = node_text(source, condition)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if condition == format!("{name} != none")
            || condition == format!("none != {name}")
            || condition == format!("not ({name} is none)")
        {
            return true;
        }
        return enclosed_by_guard(source, block, position, name);
    }
    current
        .children
        .iter()
        .filter(|child| child.span.start <= position && position <= child.span.end)
        .any(|child| enclosed_by_guard(source, child, position, name))
}
pub(crate) fn is_narrowed_text_range(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    bindings: &[TypedBinding],
) -> bool {
    let name = node_text(&unit.source, node);
    if !bindings.iter().rev().any(|binding| {
        binding.name == name
            && binding.span.start <= node.span.start
            && binding.value_type == ValueType::TextRangeOrNone
    }) {
        return false;
    }

    enclosed_by_guard(&unit.source, &unit.tree.root, node.span.start, name)
}
#[expect(
    clippy::too_many_lines,
    reason = "value inference centralizes the precedence among syntax forms and typed member families"
)]
fn infer_value_type(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    bindings: &[TypedBinding],
) -> Result<Option<ValueType>, SemanticFailure> {
    if node.kind == SyntaxKind::Literal {
        return Ok(infer_literal_type(unit, node).map(ValueType::Scalar));
    }
    if node.kind == SyntaxKind::GroupExpression {
        return match node.children.first() {
            Some(child) => infer_value_type(unit, child, bindings),
            None => Ok(None),
        };
    }
    if node.kind == SyntaxKind::UnaryExpression {
        return infer_unary_type(unit, node, bindings).map(Some);
    }
    if node.kind == SyntaxKind::BinaryExpression {
        return infer_binary_type(unit, node, bindings).map(Some);
    }
    if node.kind == SyntaxKind::TypeMembershipExpression {
        return Ok(Some(ValueType::Scalar(ScalarType::Bool)));
    }
    if node.kind == SyntaxKind::Name {
        let name = node_text(&unit.source, node);
        if let Some(binding) = bindings
            .iter()
            .rev()
            .find(|binding| binding.name == name && binding.span.start <= node.span.start)
        {
            return Ok(Some(
                if binding.value_type == ValueType::TextRangeOrNone
                    && is_narrowed_text_range(unit, node, bindings)
                {
                    ValueType::TextRange
                } else {
                    binding.value_type
                },
            ));
        }
        let resolved_encoding = lexical_scope_chain(unit, node.span.start)
            .find_map(|scope| {
                scope.symbols.get(name)?.iter().rev().find(|symbol| {
                    symbol
                        .declaration_span
                        .is_none_or(|span| span.end <= node.span.start)
                })
            })
            .map(|symbol| symbol.identity.as_str())
            .or_else(|| {
                unit.prelude.then_some(name).and_then(|name| {
                    matches!(
                        name,
                        "utf8" | "utf16-le" | "utf16-be" | "utf32-le" | "utf32-be"
                    )
                    .then_some(name)
                })
            })
            .is_some_and(|identity| {
                identity.starts_with("/core/encodings::")
                    || matches!(
                        identity,
                        "utf8" | "utf16-le" | "utf16-be" | "utf32-le" | "utf32-be"
                    )
            });
        return Ok(resolved_encoding.then_some(ValueType::Encoding));
    }
    if member_family_receiver(unit, node) {
        return Err(failure(
            &unit.source,
            "T0018",
            "member-family selections must be invoked in the same expression",
            node.span,
        ));
    }
    if node.kind == SyntaxKind::MemberExpression {
        return infer_member_value_type(unit, node, bindings);
    }
    if node.kind == SyntaxKind::CallExpression {
        if let Some(value_type) = infer_string_call_type(unit, node, bindings)? {
            return Ok(Some(value_type));
        }
        if let Some(value_type) = infer_arithmetic_family_type(unit, node, bindings)? {
            return Ok(Some(value_type));
        }
        if let Some(value_type) = infer_parse_or_radix_type(unit, node, bindings)? {
            return Ok(Some(value_type));
        }
        if let Some(value_type) = infer_integer_coercion_type(unit, node, bindings)? {
            return Ok(Some(value_type));
        }
        if let Some(callee) = node.children.first()
            && callee.kind == SyntaxKind::MemberExpression
            && let [receiver, member] = callee.children.as_slice()
            && matches!(node_text(&unit.source, member), "concat" | "join")
        {
            let receiver_type = infer_value_type(unit, receiver, bindings)?;
            if receiver_type == Some(ValueType::Scalar(ScalarType::String)) {
                return Ok(Some(ValueType::Scalar(ScalarType::String)));
            }
            return Err(failure(
                &unit.source,
                "T0013",
                format!(
                    "`.{}` requires a `string` receiver, found `{}`",
                    node_text(&unit.source, member),
                    receiver_type
                        .map_or_else(|| "unknown".to_owned(), |value_type| value_type.to_string())
                ),
                receiver.span,
            ));
        }
        if let Some(callee) = node.children.first()
            && callee.kind == SyntaxKind::Name
        {
            let name = node_text(&unit.source, callee);
            if let Some(return_type) = unit
                .functions
                .iter()
                .find(|contract| contract.name == name)
                .and_then(|contract| contract.return_type)
            {
                return Ok(Some(return_type));
            }
            return Ok(None);
        }
        return Ok(None);
    }
    Ok(None)
}

fn infer_member_value_type(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    bindings: &[TypedBinding],
) -> Result<Option<ValueType>, SemanticFailure> {
    let [receiver, member] = node.children.as_slice() else {
        return Ok(None);
    };
    if matches!(node_text(&unit.source, member), "concat" | "join") {
        return Err(failure(
            &unit.source,
            "T0018",
            "string methods are not storable values before bound methods exist",
            node.span,
        ));
    }
    let member_name = node_text(&unit.source, member);
    let receiver_type = infer_value_type(unit, receiver, bindings)?;
    if receiver_type == Some(ValueType::Scalar(ScalarType::String)) {
        let view = match member_name {
            "bytes" => Some(TextUnit::Bytes),
            "scalars" => Some(TextUnit::Scalars),
            "graphemes" => Some(TextUnit::Graphemes),
            _ => None,
        };
        if let Some(view) = view {
            return Ok(Some(ValueType::StringView(view)));
        }
    }
    if receiver_type == Some(ValueType::TextRange) {
        return Ok(match member_name {
            "text" => Some(ValueType::Scalar(ScalarType::String)),
            "bytes" => Some(ValueType::TextRangeView(TextUnit::Bytes)),
            "scalars" => Some(ValueType::TextRangeView(TextUnit::Scalars)),
            "graphemes" => Some(ValueType::TextRangeView(TextUnit::Graphemes)),
            _ => None,
        });
    }
    if matches!(receiver_type, Some(ValueType::TextRangeView(_)))
        && matches!(member_name, "start" | "end")
    {
        return Ok(Some(ValueType::Scalar(ScalarType::Int)));
    }
    if matches!(
        receiver_type,
        Some(
            ValueType::StringView(_) | ValueType::StringList | ValueType::Scalar(ScalarType::Bytes)
        )
    ) && member_name == "length"
    {
        return Ok(Some(ValueType::Scalar(ScalarType::Int)));
    }
    match (receiver_type, member_name) {
        (Some(ValueType::OverflowResult(ty)), "value")
        | (Some(ValueType::DivRemResult(ty)), "quotient" | "remainder") => {
            return Ok(Some(ValueType::Scalar(ty)));
        }
        (Some(ValueType::OverflowResult(_)), "overflowed") => {
            return Ok(Some(ValueType::Scalar(ScalarType::Bool)));
        }
        (Some(ValueType::OverflowResult(_) | ValueType::DivRemResult(_)), _) => {
            return Err(failure(
                &unit.source,
                "T0031",
                format!("result object has no member `.{member_name}`"),
                member.span,
            ));
        }
        _ => {}
    }
    if matches!(member_name, "round" | "floor" | "ceiling" | "truncate") {
        if matches!(
            receiver_type,
            Some(ValueType::Scalar(ScalarType::Float32 | ScalarType::Float64))
        ) {
            return Ok(Some(ValueType::Scalar(ScalarType::Int)));
        }
        return Err(failure(
            &unit.source,
            "T0013",
            format!("`.{member_name}` requires a floating receiver"),
            receiver.span,
        ));
    }
    if member_name != "length" {
        return Ok(None);
    }
    let receiver_type = infer_value_type(unit, receiver, bindings)?;
    if matches!(
        receiver_type,
        Some(ValueType::Scalar(ScalarType::String | ScalarType::Bytes))
    ) {
        return Ok(Some(ValueType::Scalar(ScalarType::Int)));
    }
    Err(failure(
        &unit.source,
        "T0013",
        format!(
            "`.length` requires `string` or `bytes`, found `{}`",
            receiver_type
                .map_or_else(|| "unknown".to_owned(), |value_type| value_type.to_string(),)
        ),
        receiver.span,
    ))
}

#[allow(clippy::too_many_lines)]
fn infer_string_call_type(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    bindings: &[TypedBinding],
) -> Result<Option<ValueType>, SemanticFailure> {
    let Some(callee) = node.children.first() else {
        return Ok(None);
    };
    let [receiver, member] = callee.children.as_slice() else {
        return Ok(None);
    };
    if callee.kind != SyntaxKind::MemberExpression {
        return Ok(None);
    }
    let mut subject = receiver;
    let mut family = node_text(&unit.source, member);
    let mut child = "default";
    if receiver.kind == SyntaxKind::MemberExpression
        && let [nested_subject, nested_family] = receiver.children.as_slice()
    {
        let candidate = node_text(&unit.source, nested_family);
        if matches!(
            candidate,
            "trim" | "contains" | "find" | "normalise" | "upper" | "lower"
        ) {
            subject = nested_subject;
            family = candidate;
            child = node_text(&unit.source, member);
        }
    }
    if !matches!(
        family,
        "trim"
            | "contains"
            | "find"
            | "upper"
            | "lower"
            | "normalise"
            | "case-fold"
            | "split"
            | "replace"
            | "encode"
            | "decode"
    ) {
        return Ok(None);
    }
    let subject_type = infer_value_type(unit, subject, bindings)?;
    let receiver_valid = match family {
        "decode" => subject_type == Some(ValueType::Scalar(ScalarType::Bytes)),
        _ => subject_type == Some(ValueType::Scalar(ScalarType::String)),
    };
    if !receiver_valid {
        return Err(failure(
            &unit.source,
            "T0032",
            format!("`.{family}` is not available on this receiver"),
            subject.span,
        ));
    }
    let arguments = node
        .children
        .get(1)
        .map_or(&[][..], |arguments| arguments.children.as_slice());
    let (minimum, maximum) = match (family, child) {
        ("trim", "default") | ("upper" | "lower" | "normalise" | "case-fold", _) => (0, 0),
        ("trim", "start" | "end") => (0, 1),
        ("replace", _) => (2, 2),
        _ => (1, 1),
    };
    if arguments.len() < minimum || arguments.len() > maximum {
        return Err(failure(
            &unit.source,
            "T0023",
            format!("`.{family}` received the wrong number of arguments"),
            node.span,
        ));
    }
    for argument in arguments {
        let argument = argument.children.last().unwrap_or(argument);
        let expected = if matches!(family, "encode" | "decode") {
            ValueType::Encoding
        } else {
            ValueType::Scalar(ScalarType::String)
        };
        if infer_value_type(unit, argument, bindings)? != Some(expected) {
            return Err(failure(
                &unit.source,
                "T0033",
                format!("`.{family}` received an incompatible argument"),
                argument.span,
            ));
        }
    }
    let result = match (family, child) {
        ("contains", "default" | "start" | "end") => ValueType::Scalar(ScalarType::Bool),
        ("find", "default") => ValueType::TextRangeOrNone,
        ("find", "all") => ValueType::TextRangeList,
        ("find", "count") => ValueType::Scalar(ScalarType::Int),
        ("split", "default") => ValueType::StringList,
        ("encode", "default") => ValueType::Scalar(ScalarType::Bytes),
        ("decode" | "case-fold" | "replace", "default")
        | ("trim", "default" | "start" | "end")
        | ("upper", "default" | "first" | "words")
        | ("lower", "default" | "first")
        | ("normalise", "nfc" | "nfd" | "nfkc" | "nfkd") => ValueType::Scalar(ScalarType::String),
        _ => {
            return Err(failure(
                &unit.source,
                "T0034",
                format!("`.{family}.{child}` is not available"),
                callee.span,
            ));
        }
    };
    Ok(Some(result))
}
fn infer_unary_type(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    bindings: &[TypedBinding],
) -> Result<ValueType, SemanticFailure> {
    let Some(operand_node) = node.children.last() else {
        return Err(operator_failure(
            unit,
            node,
            "unary operator requires an operand",
        ));
    };
    let Some(ValueType::Scalar(operand)) = infer_value_type(unit, operand_node, bindings)? else {
        return Err(operator_failure(
            unit,
            node,
            "unary operator requires a scalar operand",
        ));
    };

    let operator = unit.source.text()[node.span.start..operand_node.span.start].trim();
    let valid = match operator {
        "-" => operand.is_integer() || matches!(operand, ScalarType::Float32 | ScalarType::Float64),
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
#[expect(
    clippy::too_many_lines,
    reason = "family receiver, callback, argument, and result contracts remain auditable together"
)]
fn infer_parse_or_radix_type(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    bindings: &[TypedBinding],
) -> Result<Option<ValueType>, SemanticFailure> {
    let Some(callee) = node.children.first() else {
        return Ok(None);
    };
    let Some(method) = bound_method(&unit.source, callee) else {
        return Ok(None);
    };
    if matches!(
        method.family,
        MemberFamily::Coerce | MemberFamily::Arithmetic(_)
    ) {
        return Ok(None);
    }
    let arguments = node.children.get(1);
    let arguments = arguments.map_or(&[][..], |arguments| arguments.children.as_slice());
    if arguments.len() != 1 {
        return Err(failure(
            &unit.source,
            "T0023",
            format!(
                "`.{}` requires exactly one argument",
                match method.family {
                    MemberFamily::Parse => "parse",
                    MemberFamily::Radix => "radix",
                    MemberFamily::Coerce | MemberFamily::Arithmetic(_) => unreachable!(),
                }
            ),
            node.span,
        ));
    }
    let receiver = find_node_by_span(&unit.tree.root, method.receiver)
        .expect("bound method receiver belongs to this syntax tree");
    let argument = arguments[0].children.last().unwrap_or(&arguments[0]);
    if method.family == MemberFamily::Radix {
        let argument_type = infer_value_type(unit, argument, bindings)?;
        if !matches!(argument_type, Some(ValueType::Scalar(scalar)) if scalar.is_integer()) {
            return Err(failure(
                &unit.source,
                "T0024",
                "`.radix` requires an integer radix argument",
                argument.span,
            ));
        }
        let receiver_type = infer_value_type(unit, receiver, bindings)?;
        return match receiver_type {
            Some(ValueType::Scalar(ScalarType::String)) => {
                Ok(Some(ValueType::Scalar(ScalarType::Int)))
            }
            Some(ValueType::Scalar(scalar)) if scalar.is_integer() => {
                Ok(Some(ValueType::Scalar(ScalarType::String)))
            }
            _ => Err(failure(
                &unit.source,
                "T0024",
                "`.radix` requires a string or numeric receiver",
                receiver.span,
            )),
        };
    }
    let receiver_type = infer_value_type(unit, receiver, bindings)?;
    if receiver_type != Some(ValueType::Scalar(ScalarType::String)) {
        return Err(failure(
            &unit.source,
            "T0024",
            "`.parse` requires a string receiver",
            receiver.span,
        ));
    }
    let callback = arguments[0].children.last().unwrap_or(&arguments[0]);
    if callback.kind != SyntaxKind::Name {
        return Err(failure(
            &unit.source,
            "T0025",
            "`.parse` requires a statically resolvable function name",
            callback.span,
        ));
    }
    let callback_name = node_text(&unit.source, callback);
    let Some(contract) = unit.function_aliases.get(callback_name) else {
        return Err(failure(
            &unit.source,
            "T0025",
            format!("`{callback_name}` does not resolve to a parse callback"),
            callback.span,
        ));
    };
    if contract.parameters.len() != 1
        || contract.parameters[0].value_type != Some(ValueType::Scalar(ScalarType::String))
        || !matches!(contract.return_type, Some(ValueType::Scalar(_)))
    {
        return Err(failure(
            &unit.source,
            "T0026",
            format!(
                "parse callback `{callback_name}` must take one `string` value and declare a scalar return"
            ),
            callback.span,
        ));
    }
    let Some(ValueType::Scalar(result)) = contract.return_type else {
        unreachable!("checked above")
    };
    Ok(Some(if method.child == "checked" {
        ValueType::ScalarOrNone(result)
    } else {
        ValueType::Scalar(result)
    }))
}

#[allow(clippy::too_many_lines)]
fn infer_arithmetic_family_type(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    bindings: &[TypedBinding],
) -> Result<Option<ValueType>, SemanticFailure> {
    let Some(callee) = node.children.first() else {
        return Ok(None);
    };
    let Some(method) = bound_method(&unit.source, callee) else {
        return Ok(None);
    };
    let MemberFamily::Arithmetic(family) = method.family else {
        return Ok(None);
    };
    let receiver = find_node_by_span(&unit.tree.root, method.receiver)
        .expect("bound arithmetic receiver belongs to this syntax tree");
    let Some(ValueType::Scalar(receiver_type)) = infer_value_type(unit, receiver, bindings)? else {
        return Err(failure(
            &unit.source,
            "T0027",
            format!("`.{}` requires an integer receiver", family.source_name()),
            receiver.span,
        ));
    };
    if !receiver_type.is_integer() {
        return Err(failure(
            &unit.source,
            "T0027",
            format!("`.{}` requires an integer receiver", family.source_name()),
            receiver.span,
        ));
    }
    if family == ArithmeticFamily::Negate
        && !matches!(
            receiver_type,
            ScalarType::Int
                | ScalarType::Int8
                | ScalarType::Int16
                | ScalarType::Int32
                | ScalarType::Int64
                | ScalarType::Int128
        )
    {
        return Err(failure(
            &unit.source,
            "T0027",
            "`.negate` is not available on unsigned integers",
            receiver.span,
        ));
    }
    let arguments = node.children.get(1);
    let arguments = arguments.map_or(&[][..], |arguments| arguments.children.as_slice());
    let expected = usize::from(family != ArithmeticFamily::Negate);
    if arguments.len() != expected {
        return Err(failure(
            &unit.source,
            "T0023",
            format!(
                "`.{}` requires exactly {expected} argument{}",
                family.source_name(),
                if expected == 1 { "" } else { "s" }
            ),
            node.span,
        ));
    }
    if let Some(argument) = arguments.first() {
        let argument = argument.children.last().unwrap_or(argument);
        let argument_type = infer_value_type(unit, argument, bindings)?;
        let valid = if matches!(
            family,
            ArithmeticFamily::ShiftLeft | ArithmeticFamily::ShiftRight
        ) {
            matches!(argument_type, Some(ValueType::Scalar(ty)) if ty.is_integer())
        } else {
            argument_type == Some(ValueType::Scalar(receiver_type))
                || contextual_constant(&unit.source, argument, receiver_type).is_some()
        };
        if !valid {
            return Err(failure(
                &unit.source,
                "T0028",
                format!(
                    "`.{}` argument is incompatible with `{receiver_type}`",
                    family.source_name()
                ),
                argument.span,
            ));
        }
    }
    let fixed = receiver_type != ScalarType::Int;
    let child_allowed = match method.child {
        "default" => true,
        "checked" => {
            fixed
                || matches!(
                    family,
                    ArithmeticFamily::Divide
                        | ArithmeticFamily::Remainder
                        | ArithmeticFamily::DivRem
                )
        }
        "wrap" => fixed && family != ArithmeticFamily::DivRem,
        "saturate" | "overflowing" => {
            fixed
                && !matches!(
                    family,
                    ArithmeticFamily::DivRem
                        | ArithmeticFamily::ShiftLeft
                        | ArithmeticFamily::ShiftRight
                )
        }
        _ => false,
    };
    if !child_allowed {
        return Err(failure(
            &unit.source,
            "T0029",
            format!(
                "`.{}.{}` is not available on `{receiver_type}`",
                family.source_name(),
                method.child
            ),
            callee.span,
        ));
    }
    let result = if method.child == "overflowing" {
        ValueType::OverflowResult(receiver_type)
    } else if family == ArithmeticFamily::DivRem {
        if method.child == "checked" {
            return Err(failure(
                &unit.source,
                "T0030",
                "`div-rem.checked` optional result values are not yet representable",
                callee.span,
            ));
        }
        ValueType::DivRemResult(receiver_type)
    } else if method.child == "checked" {
        ValueType::ScalarOrNone(receiver_type)
    } else {
        ValueType::Scalar(receiver_type)
    };
    Ok(Some(result))
}

fn infer_integer_coercion_type(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    bindings: &[TypedBinding],
) -> Result<Option<ValueType>, SemanticFailure> {
    let Some(callee) = node.children.first() else {
        return Ok(None);
    };
    let Some((source_node, policy)) = integer_coercion_call(&unit.source, callee) else {
        if let Some(member) = obsolete_integer_coercion_member(unit, callee) {
            return Err(failure(
                &unit.source,
                "T0017",
                format!(
                    "`{member}` is not valid syntax; use `.coerce.{}`",
                    match member {
                        "checked-coerce" => "checked",
                        "wrapping-coerce" => "wrap",
                        "saturating-coerce" => "saturate",
                        _ => unreachable!("obsolete coercion members are matched above"),
                    }
                ),
                callee.span,
            ));
        }
        if let Some(chain) = invalid_coercion_policy(unit, callee) {
            return Err(failure(
                &unit.source,
                "T0010",
                format!("`{chain}` is not an available coercion policy"),
                callee.span,
            ));
        }
        return Ok(None);
    };
    let Some(ValueType::Scalar(source_type)) = infer_value_type(unit, source_node, bindings)?
    else {
        return Err(failure(
            &unit.source,
            "T0009",
            "`.coerce` requires an integer source",
            source_node.span,
        ));
    };
    if !source_type.is_integer() {
        return Err(failure(
            &unit.source,
            "T0009",
            format!(
                "`{}` requires an integer source, found `{source_type}`",
                policy.invocation_name()
            ),
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
                format!(
                    "`{}` from `{source_type}` requires one integer destination",
                    policy.invocation_name()
                ),
                node.span,
            )
        })?;
    let destination_name = node_text(&unit.source, destination_node);
    let destination = unit
        .descriptor_alias_at(destination_name, destination_node.span.start)
        .ok_or_else(|| {
            failure(
                &unit.source,
                "T0008",
                format!(
                    "`{destination_name}` is not a supported destination for `{}` from `{source_type}`",
                    policy.invocation_name()
                ),
                destination_node.span,
            )
        })?;
    if !destination.is_integer() {
        return Err(failure(
            &unit.source,
            "T0008",
            format!(
                "`{destination}` is not a supported destination for `{}` from `{source_type}`",
                policy.invocation_name()
            ),
            destination_node.span,
        ));
    }
    let result = integer_coercion_result_type(source_type, destination, policy)
        .map_err(|message| failure(&unit.source, "T0010", message, destination_node.span))?;
    Ok(Some(result))
}

fn integer_coercion_result_type(
    source: ScalarType,
    destination: ScalarType,
    policy: CoercionPolicy,
) -> Result<ValueType, String> {
    match (source, destination, policy) {
        (
            _,
            ScalarType::Int,
            CoercionPolicy::Checked | CoercionPolicy::Wrap | CoercionPolicy::Saturate,
        ) => Err(format!(
            "`.coerce.{}` from `{source}` requires a fixed-width integer destination",
            policy.source_name()
        )),
        (_, _, CoercionPolicy::Checked) => Ok(ValueType::ScalarOrNone(destination)),
        (_, _, CoercionPolicy::Default | CoercionPolicy::Wrap | CoercionPolicy::Saturate) => {
            Ok(ValueType::Scalar(destination))
        }
    }
}

pub(crate) fn bound_method(source: &SourceFile, callee: &SyntaxNode) -> Option<BoundMethod> {
    if callee.kind != SyntaxKind::MemberExpression {
        return None;
    }
    let [receiver, member] = callee.children.as_slice() else {
        return None;
    };
    let member_name = node_text(source, member);
    let direct = match member_name {
        "coerce" => Some((MemberFamily::Coerce, "default")),
        "parse" => Some((MemberFamily::Parse, "default")),
        "radix" => Some((MemberFamily::Radix, "default")),
        name => ArithmeticFamily::from_source_name(name)
            .map(|family| (MemberFamily::Arithmetic(family), "default")),
    };
    if let Some((family, child)) = direct {
        return Some(BoundMethod {
            receiver: receiver.span,
            family,
            child,
        });
    }
    if receiver.kind != SyntaxKind::MemberExpression {
        return None;
    }
    let [source_node, family_node] = receiver.children.as_slice() else {
        return None;
    };
    let selection = match (node_text(source, family_node), member_name) {
        ("coerce", "checked") => (MemberFamily::Coerce, "checked"),
        ("coerce", "wrap") => (MemberFamily::Coerce, "wrap"),
        ("coerce", "saturate") => (MemberFamily::Coerce, "saturate"),
        ("parse", "checked") => (MemberFamily::Parse, "checked"),
        (family, child @ ("checked" | "wrap" | "saturate" | "overflowing")) => {
            let child = match child {
                "checked" => "checked",
                "wrap" => "wrap",
                "saturate" => "saturate",
                "overflowing" => "overflowing",
                _ => unreachable!(),
            };
            (
                MemberFamily::Arithmetic(ArithmeticFamily::from_source_name(family)?),
                child,
            )
        }
        _ => return None,
    };
    Some(BoundMethod {
        receiver: source_node.span,
        family: selection.0,
        child: selection.1,
    })
}

/// Resolves the canonical `.coerce` callable family and its selected policy child.
///
/// The returned policy is shared semantic metadata for analysis and lowering; the
/// Rust helper names used after family erasure are not independent source members.
pub(crate) fn integer_coercion_call<'a>(
    source: &SourceFile,
    callee: &'a SyntaxNode,
) -> Option<(&'a SyntaxNode, CoercionPolicy)> {
    let method = bound_method(source, callee)?;
    if method.family != MemberFamily::Coerce {
        return None;
    }
    let policy = match method.child {
        "default" => CoercionPolicy::Default,
        child => CoercionPolicy::from_member(child)?,
    };
    let receiver = callee.children.first()?;
    let source_node = if method.child == "default" {
        receiver
    } else {
        receiver.children.first()?
    };
    Some((source_node, policy))
}

fn invalid_coercion_policy(unit: &SemanticUnit, callee: &SyntaxNode) -> Option<String> {
    (coercion_family_receiver(unit, callee)
        && integer_coercion_call(&unit.source, callee).is_none())
    .then(|| {
        let callee_text = node_text(&unit.source, callee);
        let family_start = callee_text.find(".coerce").unwrap_or(0);
        callee_text[family_start..].to_owned()
    })
}

fn coercion_family_receiver(unit: &SemanticUnit, node: &SyntaxNode) -> bool {
    let [receiver, member] = node.children.as_slice() else {
        return false;
    };
    node.kind == SyntaxKind::MemberExpression
        && (node_text(&unit.source, member) == "coerce" || coercion_family_receiver(unit, receiver))
}

fn member_family_receiver(unit: &SemanticUnit, node: &SyntaxNode) -> bool {
    let [receiver, member] = node.children.as_slice() else {
        return false;
    };
    if node_text(&unit.source, member) == "remainder"
        && matches!(
            infer_value_type(unit, receiver, &unit.typed_bindings),
            Ok(Some(ValueType::DivRemResult(_)))
        )
    {
        return false;
    }
    node.kind == SyntaxKind::MemberExpression
        && (matches!(
            node_text(&unit.source, member),
            "coerce"
                | "parse"
                | "radix"
                | "add"
                | "subtract"
                | "multiply"
                | "divide"
                | "remainder"
                | "div-rem"
                | "negate"
                | "shift-left"
                | "shift-right"
        ) || member_family_receiver(unit, receiver))
}

fn obsolete_integer_coercion_member<'a>(
    unit: &'a SemanticUnit,
    callee: &'a SyntaxNode,
) -> Option<&'a str> {
    let [_, member] = callee.children.as_slice() else {
        return None;
    };
    (callee.kind == SyntaxKind::MemberExpression)
        .then(|| node_text(&unit.source, member))
        .filter(|member| {
            matches!(
                *member,
                "checked-coerce" | "wrapping-coerce" | "saturating-coerce"
            )
        })
}

#[expect(
    clippy::too_many_lines,
    reason = "binary inference keeps operator precedence, optional equality, and numeric promotion auditable"
)]
fn infer_binary_type(
    unit: &SemanticUnit,
    node: &SyntaxNode,
    bindings: &[TypedBinding],
) -> Result<ValueType, SemanticFailure> {
    let [left_node, right_node] = node.children.as_slice() else {
        return Err(operator_failure(
            unit,
            node,
            "binary operator requires two operands",
        ));
    };
    let left = infer_value_type(unit, left_node, bindings)?;
    let right = infer_value_type(unit, right_node, bindings)?;
    let operator = unit.source.text()[left_node.span.end..right_node.span.start].trim();
    if operator == "is" {
        return Ok(ValueType::Scalar(ScalarType::Bool));
    }
    if matches!(operator, "==" | "!=")
        && ((matches!(
            left,
            Some(ValueType::ScalarOrNone(_) | ValueType::TextRangeOrNone)
        ) && node_text(&unit.source, right_node).trim() == "none")
            || (matches!(
                right,
                Some(ValueType::ScalarOrNone(_) | ValueType::TextRangeOrNone)
            ) && node_text(&unit.source, left_node).trim() == "none"))
    {
        return Ok(ValueType::Scalar(ScalarType::Bool));
    }
    let comparison = matches!(operator, "==" | "!=" | "<" | "<=" | ">" | ">=");
    let contextual_numeric = matches!(
        operator,
        "+" | "-" | "*" | "/" | "%" | "&" | "^" | "|" | "==" | "!=" | "<" | "<=" | ">" | ">="
    );
    if contextual_numeric
        && let Some(ValueType::Scalar(left_type)) = left
        && is_numeric(left_type)
        && contextual_constant(&unit.source, right_node, left_type)
            .transpose()?
            .is_some()
    {
        return Ok(ValueType::Scalar(if comparison {
            ScalarType::Bool
        } else {
            left_type
        }));
    }
    if contextual_numeric
        && let Some(ValueType::Scalar(right_type)) = right
        && is_numeric(right_type)
        && contextual_constant(&unit.source, left_node, right_type)
            .transpose()?
            .is_some()
    {
        return Ok(ValueType::Scalar(if comparison {
            ScalarType::Bool
        } else {
            right_type
        }));
    }
    let (Some(ValueType::Scalar(left)), Some(ValueType::Scalar(right))) = (left, right) else {
        return Err(operator_failure(
            unit,
            node,
            "operator requires scalar operands",
        ));
    };
    let same = left == right;
    if contextual_numeric && left != right && left.is_integer() && right.is_integer() {
        if contextual_constant(&unit.source, right_node, right).is_some() {
            contextual_constant(&unit.source, right_node, left).expect(
                "integer constant expression remains contextual across integer destinations",
            )?;
        }
        if contextual_constant(&unit.source, left_node, left).is_some() {
            contextual_constant(&unit.source, left_node, right).expect(
                "integer constant expression remains contextual across integer destinations",
            )?;
        }
    }
    if contextual_numeric && left != right && left.is_integer() && right.is_integer() {
        return Ok(ValueType::Scalar(if comparison {
            ScalarType::Bool
        } else {
            promoted_integer_type(left, right)
        }));
    }
    let numeric =
        |ty: ScalarType| ty.is_integer() || matches!(ty, ScalarType::Float32 | ScalarType::Float64);
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

fn infer_literal_type(unit: &SemanticUnit, node: &SyntaxNode) -> Option<ScalarType> {
    infer_literal_type_from_source(&unit.source, node)
}

fn infer_literal_type_from_source(source: &SourceFile, node: &SyntaxNode) -> Option<ScalarType> {
    if node.kind == SyntaxKind::UnaryExpression {
        return node
            .children
            .last()
            .and_then(|child| infer_literal_type_from_source(source, child));
    }
    if node.kind != SyntaxKind::Literal {
        return None;
    }

    let text = node_text(source, node);
    match text {
        "true" | "false" => Some(ScalarType::Bool),
        value if value.starts_with("b'") => Some(ScalarType::Bytes),
        value if value.starts_with(['\'', '"', '>']) => Some(ScalarType::String),
        value if value.contains('.') => Some(ScalarType::Float64),
        _ => Some(ScalarType::Int),
    }
}

pub(crate) fn contextual_constant(
    source: &SourceFile,
    node: &SyntaxNode,
    destination: ScalarType,
) -> Option<Result<ContextualConstant, SemanticFailure>> {
    if !is_numeric(destination) {
        return None;
    }
    contextual_constant_value(source, node, destination).map(|result| {
        result.and_then(|value| {
            match &value {
                ContextualConstant::Integer(integer) => {
                    check_integer_range(source, destination, integer, node.span)?;
                }
                ContextualConstant::Float32(value) if !value.is_finite() => {
                    return Err(invalid_floating_constant(source, destination, node.span));
                }
                ContextualConstant::Float64(value) if !value.is_finite() => {
                    return Err(invalid_floating_constant(source, destination, node.span));
                }
                ContextualConstant::Float32(_) | ContextualConstant::Float64(_) => {}
            }
            Ok(value)
        })
    })
}
fn small_int_storage(
    unit: &SemanticUnit,
    value: &SyntaxNode,
    inferred: Option<ValueType>,
) -> Option<ScalarType> {
    if let Some(Ok(ContextualConstant::Integer(integer))) =
        contextual_constant(&unit.source, value, ScalarType::Int)
        && integer.to_i64().is_some()
    {
        return Some(ScalarType::Int64);
    }
    matches!(
        inferred,
        Some(ValueType::Scalar(
            ScalarType::Int8
                | ScalarType::Int16
                | ScalarType::Int32
                | ScalarType::Int64
                | ScalarType::Uint8
                | ScalarType::Uint16
                | ScalarType::Uint32
        ))
    )
    .then_some(ScalarType::Int64)
}

fn contextual_constant_value(
    source: &SourceFile,
    node: &SyntaxNode,
    destination: ScalarType,
) -> Option<Result<ContextualConstant, SemanticFailure>> {
    let result = match node.kind {
        SyntaxKind::GroupExpression => {
            return node
                .children
                .first()
                .and_then(|child| contextual_constant_value(source, child, destination));
        }
        SyntaxKind::UnaryExpression => {
            let operand = node.children.last()?;
            let value = contextual_constant_value(source, operand, destination)?;
            value.map(|value| match value {
                ContextualConstant::Integer(value) => ContextualConstant::Integer(-value),
                ContextualConstant::Float32(value) => ContextualConstant::Float32(-value),
                ContextualConstant::Float64(value) => ContextualConstant::Float64(-value),
            })
        }
        SyntaxKind::BinaryExpression => {
            let [left, right] = node.children.as_slice() else {
                return None;
            };
            let operator = source.text()[left.span.end..right.span.start].trim();
            let valid = if destination.is_integer() {
                matches!(
                    operator,
                    "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "<<" | ">>"
                )
            } else {
                matches!(operator, "+" | "-" | "*" | "/" | "%")
            };
            if !valid {
                return None;
            }
            let left = contextual_constant_value(source, left, destination)?;
            let right = contextual_constant_value(source, right, destination)?;
            match (left, right) {
                (Ok(ContextualConstant::Integer(left)), Ok(ContextualConstant::Integer(right))) => {
                    fold_integer_constant(source, node.span, operator, left, right)
                }
                (Ok(ContextualConstant::Float32(left)), Ok(ContextualConstant::Float32(right))) => {
                    Ok(ContextualConstant::Float32(fold_float32_constant(
                        operator, left, right,
                    )))
                }
                (Ok(ContextualConstant::Float64(left)), Ok(ContextualConstant::Float64(right))) => {
                    Ok(ContextualConstant::Float64(fold_float64_constant(
                        operator, left, right,
                    )))
                }
                (Err(error), _) | (_, Err(error)) => Err(error),
                _ => return None,
            }
        }
        SyntaxKind::Literal
            if infer_literal_type_from_source(source, node).is_some_and(is_numeric) =>
        {
            contextual_literal(source, node, destination)
        }
        _ => return None,
    };
    Some(result)
}

fn contextual_literal(
    source: &SourceFile,
    node: &SyntaxNode,
    destination: ScalarType,
) -> Result<ContextualConstant, SemanticFailure> {
    let text = node_text(source, node).replace('_', "");
    let decimal = text.contains('.');
    if destination.is_integer() {
        let value = if decimal {
            let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
            if !fraction.chars().all(|digit| digit == '0') {
                return Err(failure(
                    source,
                    "T0003",
                    format!("constant `{text}` is not an exact `{destination}` value"),
                    node.span,
                ));
            }
            BigInt::parse_bytes(whole.as_bytes(), 10).expect("validated decimal integer constant")
        } else {
            parse_integer_source_text(source, node).expect("validated integer constant")
        };
        Ok(ContextualConstant::Integer(value))
    } else if decimal {
        if destination == ScalarType::Float32 {
            let value = text
                .parse::<f32>()
                .map_err(|_| invalid_floating_constant(source, destination, node.span))?;
            Ok(ContextualConstant::Float32(value))
        } else {
            let value = text
                .parse::<f64>()
                .map_err(|_| invalid_floating_constant(source, destination, node.span))?;
            Ok(ContextualConstant::Float64(value))
        }
    } else {
        let integer =
            parse_integer_source_text(source, node).expect("validated whole-number constant");
        if destination == ScalarType::Float32 {
            let value = integer
                .to_f32()
                .filter(|value| BigInt::from_f32(*value).as_ref() == Some(&integer))
                .ok_or_else(|| invalid_floating_constant(source, destination, node.span))?;
            Ok(ContextualConstant::Float32(value))
        } else {
            let value = integer
                .to_f64()
                .filter(|value| BigInt::from_f64(*value).as_ref() == Some(&integer))
                .ok_or_else(|| invalid_floating_constant(source, destination, node.span))?;
            Ok(ContextualConstant::Float64(value))
        }
    }
}

fn invalid_floating_constant(
    source: &SourceFile,
    destination: ScalarType,
    span: Span,
) -> SemanticFailure {
    failure(
        source,
        "T0003",
        format!("constant is not a finite exact `{destination}` value"),
        span,
    )
}

fn fold_integer_constant(
    source: &SourceFile,
    span: Span,
    operator: &str,
    left: BigInt,
    right: BigInt,
) -> Result<ContextualConstant, SemanticFailure> {
    let value = match operator {
        "+" => left + right,
        "-" => left - right,
        "*" => left * right,
        "/" if right != BigInt::from(0_u8) => {
            let quotient = &left / &right;
            let remainder = &left % &right;
            if remainder < BigInt::from(0_u8) {
                if right < BigInt::from(0_u8) {
                    quotient + 1
                } else {
                    quotient - 1
                }
            } else {
                quotient
            }
        }
        "%" if right != BigInt::from(0_u8) => {
            let remainder = &left % &right;
            if remainder < BigInt::from(0_u8) {
                if right < BigInt::from(0_u8) {
                    remainder - right
                } else {
                    remainder + right
                }
            } else {
                remainder
            }
        }
        "&" => left & right,
        "|" => left | right,
        "^" => left ^ right,
        "<<" | ">>" => {
            let Some(count) = right.to_usize() else {
                return Err(failure(
                    source,
                    "T0011",
                    "constant shift count cannot be represented on this target",
                    span,
                ));
            };
            if operator == "<<" {
                left << count
            } else {
                left >> count
            }
        }
        _ => {
            return Err(failure(
                source,
                "T0011",
                "invalid constant arithmetic",
                span,
            ));
        }
    };
    Ok(ContextualConstant::Integer(value))
}

fn fold_float32_constant(operator: &str, left: f32, right: f32) -> f32 {
    match operator {
        "+" => left + right,
        "-" => left - right,
        "*" => left * right,
        "/" => left / right,
        "%" => left % right,
        _ => unreachable!("validated constant floating operator"),
    }
}

fn fold_float64_constant(operator: &str, left: f64, right: f64) -> f64 {
    match operator {
        "+" => left + right,
        "-" => left - right,
        "*" => left * right,
        "/" => left / right,
        "%" => left % right,
        _ => unreachable!("validated constant floating operator"),
    }
}

fn parse_integer_source_text(source: &SourceFile, node: &SyntaxNode) -> Option<BigInt> {
    let compact = source.text()[node.span.start..node.span.end]
        .chars()
        .filter(|character| !character.is_whitespace() && *character != '_')
        .collect::<String>();
    let (negative, digits) = compact
        .strip_prefix('-')
        .map_or((false, compact.as_str()), |digits| (true, digits));
    let digits = digits.strip_prefix('+').unwrap_or(digits);
    let (radix, digits) = if let Some(digits) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        (16, digits)
    } else if let Some(digits) = digits
        .strip_prefix("0o")
        .or_else(|| digits.strip_prefix("0O"))
    {
        (8, digits)
    } else if let Some(digits) = digits
        .strip_prefix("0b")
        .or_else(|| digits.strip_prefix("0B"))
    {
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

pub(crate) fn promoted_integer_type(left: ScalarType, right: ScalarType) -> ScalarType {
    if left == ScalarType::Int || right == ScalarType::Int {
        return ScalarType::Int;
    }
    let left_bounds = scalar_integer_bounds(left).expect("integer operand has bounds");
    let right_bounds = scalar_integer_bounds(right).expect("integer operand has bounds");
    [
        ScalarType::Int8,
        ScalarType::Uint8,
        ScalarType::Int16,
        ScalarType::Uint16,
        ScalarType::Int32,
        ScalarType::Uint32,
        ScalarType::Int64,
        ScalarType::Uint64,
        ScalarType::Int128,
        ScalarType::Uint128,
    ]
    .into_iter()
    .find(|candidate| {
        let bounds = scalar_integer_bounds(*candidate).expect("fixed integer has bounds");
        bounds.0 <= left_bounds.0
            && bounds.0 <= right_bounds.0
            && bounds.1 >= left_bounds.1
            && bounds.1 >= right_bounds.1
    })
    .unwrap_or(ScalarType::Int)
}

fn scalar_integer_bounds(ty: ScalarType) -> Option<(BigInt, BigInt)> {
    match ty {
        ScalarType::Int8 => Some(integer_bounds(8, true)),
        ScalarType::Int16 => Some(integer_bounds(16, true)),
        ScalarType::Int32 => Some(integer_bounds(32, true)),
        ScalarType::Int64 => Some(integer_bounds(64, true)),
        ScalarType::Int128 => Some(integer_bounds(128, true)),
        ScalarType::Uint8 => Some(integer_bounds(8, false)),
        ScalarType::Uint16 => Some(integer_bounds(16, false)),
        ScalarType::Uint32 => Some(integer_bounds(32, false)),
        ScalarType::Uint64 => Some(integer_bounds(64, false)),
        ScalarType::Uint128 => Some(integer_bounds(128, false)),
        _ => None,
    }
}

fn collect_lexical_scopes(
    unit: &SemanticUnit,
    namespaces: &BTreeMap<String, Namespace>,
    globals: &BTreeMap<String, Symbol>,
) -> Result<Vec<LexicalScope>, SemanticFailure> {
    let mut scopes = Vec::new();
    for node in &unit.tree.root.children {
        match node.kind {
            SyntaxKind::FunctionDeclaration => {
                add_lexical_scope(unit, namespaces, globals, &mut scopes, node, None, true)?;
            }
            SyntaxKind::Block => {
                add_lexical_scope(unit, namespaces, globals, &mut scopes, node, None, false)?;
            }
            _ => {}
        }
    }
    Ok(scopes)
}

fn add_lexical_scope(
    unit: &SemanticUnit,
    namespaces: &BTreeMap<String, Namespace>,
    globals: &BTreeMap<String, Symbol>,
    scopes: &mut Vec<LexicalScope>,
    node: &SyntaxNode,
    parent: Option<usize>,
    function_body: bool,
) -> Result<usize, SemanticFailure> {
    let index = scopes.len();
    scopes.push(LexicalScope {
        span: node.span,
        parent,
        symbols: BTreeMap::new(),
    });
    if node.kind == SyntaxKind::Block {
        populate_scope(unit, namespaces, globals, scopes, index, node)?;
        return Ok(index);
    }
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
                populate_scope(unit, namespaces, globals, scopes, index, child)?;
            }
            SyntaxKind::Block => {
                add_lexical_scope(unit, namespaces, globals, scopes, child, Some(index), false)?;
            }
            _ if function_body => {
                populate_node(unit, namespaces, globals, scopes, index, child)?;
            }
            _ => {}
        }
    }
    Ok(index)
}

fn populate_scope(
    unit: &SemanticUnit,
    namespaces: &BTreeMap<String, Namespace>,
    globals: &BTreeMap<String, Symbol>,
    scopes: &mut Vec<LexicalScope>,
    index: usize,
    block: &SyntaxNode,
) -> Result<(), SemanticFailure> {
    for node in &block.children {
        populate_node(unit, namespaces, globals, scopes, index, node)?;
    }
    Ok(())
}

#[expect(
    clippy::too_many_lines,
    reason = "lexical scope construction handles each syntax-owned scope in one traversal"
)]
fn populate_node(
    unit: &SemanticUnit,
    namespaces: &BTreeMap<String, Namespace>,
    globals: &BTreeMap<String, Symbol>,
    scopes: &mut Vec<LexicalScope>,
    index: usize,
    node: &SyntaxNode,
) -> Result<(), SemanticFailure> {
    match node.kind {
        SyntaxKind::Binding => populate_binding(unit, scopes, index, node)?,
        SyntaxKind::Assignment => {
            populate_assignment(unit, namespaces, globals, scopes, index, node)?;
        }

        SyntaxKind::ImportDeclaration => {
            populate_imports(unit, namespaces, scopes, index, node)?;
        }
        SyntaxKind::FunctionDeclaration => {
            if let Some(name) = declaration_name(node, &unit.source) {
                insert_local(unit, scopes, index, name, node.span)?;
            }
            add_lexical_scope(unit, namespaces, globals, scopes, node, Some(index), true)?;
        }
        SyntaxKind::Block => {
            add_lexical_scope(unit, namespaces, globals, scopes, node, Some(index), false)?;
        }
        SyntaxKind::ForStatement => {
            let loop_index = scopes.len();
            scopes.push(LexicalScope {
                span: node.span,
                parent: Some(index),
                symbols: BTreeMap::new(),
            });
            if let Some(first) = node.children.first() {
                if first.kind == SyntaxKind::ForTarget {
                    for name in &first.children {
                        insert_local(
                            unit,
                            scopes,
                            loop_index,
                            node_text(&unit.source, name).to_owned(),
                            name.span,
                        )?;
                    }
                } else {
                    populate_node(unit, namespaces, globals, scopes, loop_index, first)?;
                }
            }
            if let Some(block) = node.children.last()
                && block.kind == SyntaxKind::Block
            {
                add_lexical_scope(
                    unit,
                    namespaces,
                    globals,
                    scopes,
                    block,
                    Some(loop_index),
                    false,
                )?;
            }
        }
        SyntaxKind::CatchClause => {
            let catch_index = scopes.len();
            scopes.push(LexicalScope {
                span: node.span,
                parent: Some(index),
                symbols: BTreeMap::new(),
            });
            if let Some(block) = node.children.last()
                && block.kind == SyntaxKind::Block
            {
                add_lexical_scope(
                    unit,
                    namespaces,
                    globals,
                    scopes,
                    block,
                    Some(catch_index),
                    false,
                )?;
            }
        }
        SyntaxKind::ElseClause => {
            for child in &node.children {
                if child.kind == SyntaxKind::Block {
                    add_lexical_scope(
                        unit,
                        namespaces,
                        globals,
                        scopes,
                        child,
                        Some(index),
                        false,
                    )?;
                }
            }
        }
        _ => {
            for child in &node.children {
                if child.kind == SyntaxKind::Block {
                    add_lexical_scope(
                        unit,
                        namespaces,
                        globals,
                        scopes,
                        child,
                        Some(index),
                        false,
                    )?;
                } else if matches!(
                    child.kind,
                    SyntaxKind::ElseClause | SyntaxKind::CatchClause | SyntaxKind::FinallyClause
                ) {
                    populate_node(unit, namespaces, globals, scopes, index, child)?;
                }
            }
        }
    }
    Ok(())
}

fn populate_binding(
    unit: &SemanticUnit,
    scopes: &mut [LexicalScope],
    index: usize,
    node: &SyntaxNode,
) -> Result<(), SemanticFailure> {
    let Some(declaration) = declaration_from_syntax(unit, node) else {
        return Ok(());
    };
    if declaration.global {
        return Ok(());
    }
    let typed_replacement = node
        .children
        .iter()
        .any(|child| child.kind == SyntaxKind::TypeExpression)
        && scopes[index].symbols.contains_key(&declaration.name);
    if typed_replacement {
        insert_local_replacement(unit, scopes, index, declaration.name, node.span);
        Ok(())
    } else {
        insert_local(unit, scopes, index, declaration.name, node.span)
    }
}

fn populate_assignment(
    unit: &SemanticUnit,
    namespaces: &BTreeMap<String, Namespace>,
    globals: &BTreeMap<String, Symbol>,
    scopes: &mut [LexicalScope],
    index: usize,
    node: &SyntaxNode,
) -> Result<(), SemanticFailure> {
    let Some(declaration) = declaration_from_syntax(unit, node) else {
        return Ok(());
    };
    let typed_declaration = node
        .children
        .iter()
        .any(|child| child.kind == SyntaxKind::TypeExpression);
    if declaration.global {
        return Ok(());
    }
    if typed_declaration {
        insert_local_replacement(unit, scopes, index, declaration.name, node.span);
        return Ok(());
    }
    if local_binding_exists(scopes, index, &declaration.name) {
        return Ok(());
    }
    let namespace_binding = globals
        .get(&declaration.name)
        .filter(|symbol| symbol.kind == SymbolKind::Binding)
        .or_else(|| {
            namespace_chain(&unit.namespace).find_map(|path| {
                namespaces
                    .get(&path)
                    .and_then(|scope| scope.symbols.get(&declaration.name))
                    .filter(|symbol| symbol.kind == SymbolKind::Binding)
            })
        });
    if let Some(symbol) = namespace_binding {
        let name = node
            .children
            .iter()
            .find(|child| child.kind == SyntaxKind::Name)
            .expect("ordinary assignment has a name");
        if symbol
            .declaration_span
            .is_some_and(|span| declaration_is_constant_in_unit(unit, span))
        {
            return Err(failure(
                &unit.source,
                "S2022",
                format!(
                    "constant binding `{}` cannot be reassigned",
                    declaration.name
                ),
                name.span,
            ));
        }
        return Err(SemanticFailure {
            source: unit.source.clone(),
            diagnostics: vec![
                Diagnostic::error(
                    "S2021",
                    format!(
                        "plain assignment cannot replace namespace binding `{}`",
                        declaration.name
                    ),
                    name.span,
                )
                .with_help(format!(
                    "pass `{}` as a parameter and return changes, or declare it `constant` if it never varies",
                    declaration.name
                )),
            ],
        });
    }
    insert_local(unit, scopes, index, declaration.name, node.span)
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
            let bindings = call_site_bindings(unit, Some(contract));
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

#[expect(
    clippy::too_many_lines,
    reason = "flow validation keeps every statement transition in one exhaustive dispatch"
)]
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
        SyntaxKind::ThrowStatement => Ok(false),
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
        SyntaxKind::TryStatement => {
            let try_falls_through = if let Some(block) = statement.children.first() {
                validate_flow_block(unit, block, contract, bindings, loop_depth, unreachable)?
            } else {
                true
            };
            let mut catch_falls_through = false;
            for clause in statement
                .children
                .iter()
                .filter(|child| child.kind == SyntaxKind::CatchClause)
            {
                if let Some(block) = clause
                    .children
                    .iter()
                    .find(|child| child.kind == SyntaxKind::Block)
                {
                    catch_falls_through |= validate_flow_block(
                        unit,
                        block,
                        contract,
                        bindings,
                        loop_depth,
                        unreachable,
                    )?;
                }
            }
            if let Some(finally) = statement
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::FinallyClause)
                .and_then(|clause| clause.children.first())
                && !validate_flow_block(unit, finally, contract, bindings, loop_depth, unreachable)?
            {
                return Ok(false);
            }
            Ok(try_falls_through || catch_falls_through)
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
            let mut loop_bindings = bindings.to_vec();
            if statement.children.len() == 4 {
                validate_bool_condition(unit, &statement.children[1], bindings)?;
            } else if let [target, collection, _block] = statement.children.as_slice() {
                let collection_type = infer_value_type(unit, collection, bindings)?;
                if !matches!(
                    collection_type,
                    Some(ValueType::Scalar(ScalarType::String | ScalarType::Bytes))
                ) {
                    return Err(failure(
                        &unit.source,
                        "T0016",
                        "collection iteration requires `string` or `bytes`",
                        collection.span,
                    ));
                }
                if target.children.len() != 1 {
                    return Err(failure(
                        &unit.source,
                        "T0016",
                        "string and bytes iteration require exactly one target",
                        target.span,
                    ));
                }
                let item_type = if collection_type == Some(ValueType::Scalar(ScalarType::Bytes)) {
                    ValueType::Scalar(ScalarType::Uint8)
                } else {
                    ValueType::Scalar(ScalarType::String)
                };
                loop_bindings.extend(target.children.iter().map(|name| TypedBinding {
                    name: node_text(&unit.source, name).to_owned(),
                    span: name.span,
                    value_type: item_type,
                    destination_arms: Vec::new(),
                    storage_type: None,
                    mutable: false,
                }));
            }
            if let Some(block) = statement
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Block)
            {
                validate_flow_block(
                    unit,
                    block,
                    contract,
                    &loop_bindings,
                    loop_depth + 1,
                    unreachable,
                )?;
            }
            Ok(true)
        }
        SyntaxKind::PostfixExpression => {
            let Some(operand) = statement.children.first() else {
                return Ok(true);
            };
            if operand.kind != SyntaxKind::Name
                || !matches!(
                    infer_value_type(unit, operand, bindings)?,
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
        infer_value_type(unit, condition, bindings)?,
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
            let Some(actual) = infer_value_type(unit, value, bindings)? else {
                return Err(failure(
                    &unit.source,
                    "T0015",
                    format!("function `{}` must return `{expected}`", contract.name),
                    value.span,
                ));
            };
            validate_value_destination(
                &unit.source,
                &contract.name,
                expected,
                actual,
                value,
                "T0015",
            )
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
        if let Some(existing) = scopes[index]
            .symbols
            .get(&import.alias)
            .and_then(|symbols| symbols.last())
        {
            if existing.identity == export.identity {
                continue;
            }
            return Err(failure(
                &unit.source,
                "S2011",
                format!("import `{}` collides; use an alias", import.alias),
                import.span,
            ));
        }
        scopes[index].symbols.insert(import.alias, vec![export]);
    }
    Ok(())
}

fn local_binding_exists(scopes: &[LexicalScope], mut index: usize, name: &str) -> bool {
    loop {
        let scope = &scopes[index];
        if scope.symbols.contains_key(name) {
            return true;
        }
        let Some(parent) = scope.parent else {
            return false;
        };
        index = parent;
    }
}

fn insert_local(
    unit: &SemanticUnit,
    scopes: &mut [LexicalScope],
    index: usize,
    name: String,
    span: Span,
) -> Result<(), SemanticFailure> {
    let scope = &mut scopes[index];
    if scope.symbols.contains_key(&name) {
        return Err(failure(
            &unit.source,
            "S2012",
            format!("duplicate binding `{name}` in the same lexical scope"),
            span,
        ));
    }
    insert_local_replacement(unit, scopes, index, name, span);
    Ok(())
}

fn insert_local_replacement(
    unit: &SemanticUnit,
    scopes: &mut [LexicalScope],
    index: usize,
    name: String,
    span: Span,
) {
    scopes[index]
        .symbols
        .entry(name.clone())
        .or_default()
        .push(Symbol {
            identity: format!("{}::scope{index}::{name}@{}", unit.namespace, span.start),
            name,
            namespace: unit.namespace.clone(),
            visibility: Visibility::Private,
            global: false,
            constant: false,
            kind: SymbolKind::Binding,
            declaration_span: Some(span),
        });
}

fn lexical_scope_index_at(unit: &SemanticUnit, offset: usize) -> Option<usize> {
    unit.scopes
        .iter()
        .enumerate()
        .filter(|(_, scope)| scope.span.start <= offset && offset < scope.span.end)
        .min_by_key(|(_, scope)| scope.span.end - scope.span.start)
        .map(|(index, _)| index)
}

fn lexical_scope_chain(unit: &SemanticUnit, offset: usize) -> impl Iterator<Item = &LexicalScope> {
    let mut current = lexical_scope_index_at(unit, offset);
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
    const PRELUDE: [(&str, &str, &str); 12] = [
        ("print", "/core/output::print", "/core/output"),
        ("int", "/core/types::int", "/core/types"),
        ("float", "/core/types::float", "/core/types"),
        ("bool", "/core/types::bool", "/core/types"),
        ("string", "/core/types::string", "/core/types"),
        ("bytes", "/core/types::bytes", "/core/types"),
        ("none", "/core/types::none", "/core/types"),
        ("utf8", "/core/encodings::utf8", "/core/encodings"),
        ("utf16-le", "/core/encodings::utf16-le", "/core/encodings"),
        ("utf16-be", "/core/encodings::utf16-be", "/core/encodings"),
        ("utf32-le", "/core/encodings::utf32-le", "/core/encodings"),
        ("utf32-be", "/core/encodings::utf32-be", "/core/encodings"),
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
                    visibility: Visibility::Public,
                    global: false,
                    constant: name != "print",
                    kind: if name == "print" {
                        SymbolKind::Function
                    } else if identity.starts_with("/core/encodings::") {
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

fn bootstrap_descriptor_constructs() -> BTreeMap<String, Symbol> {
    ScalarType::SOURCE_NAMES
        .into_iter()
        .map(|(source_name, ty)| {
            let name = source_name.to_owned();
            (
                name.clone(),
                Symbol {
                    identity: format!("/core/types::{}", ty.source_name()),
                    name,
                    namespace: "/core/types".to_owned(),
                    visibility: Visibility::Public,
                    global: false,
                    constant: false,
                    kind: SymbolKind::TypeDescriptor,
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
        namespace_with_objects("/core/output", ["print"], SymbolKind::Function),
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
        "overflow-result".to_owned(),
        "div-rem-result".to_owned(),
    ];
    types.extend(
        TypeCategory::ABSTRACT_SOURCE_NAMES
            .into_iter()
            .map(|(name, _)| name.to_owned()),
    );
    for prefix in ["int", "uint"] {
        for width in [8, 16, 32, 64, 128] {
            types.push(format!("{prefix}{width}"));
        }
    }
    namespaces.insert(
        "/core/types".to_owned(),
        namespace_with_objects(
            "/core/types",
            types.iter().map(std::string::String::as_str),
            SymbolKind::TypeDescriptor,
        ),
    );
    let mut errors = namespace_with_objects(
        "/core/errors",
        [
            "arithmetic-overflow",
            "division-by-zero",
            "integer-conversion-overflow",
            "negative-shift-count",
            "coercion-error",
            "decode-error",
        ],
        SymbolKind::ErrorObject,
    );
    errors.symbols.insert(
        "error".to_owned(),
        compiler_owned_object("/core/errors", "error", SymbolKind::Interface),
    );
    namespaces.insert("/core/errors".to_owned(), errors);
    namespaces.insert("/core/collections".to_owned(), Namespace::default());
    namespaces
}

fn validate_constant_reassignment(package: &SemanticPackage) -> Result<(), SemanticFailure> {
    fn visit_declarations(
        package: &SemanticPackage,
        unit: &SemanticUnit,
        node: &SyntaxNode,
    ) -> Result<(), SemanticFailure> {
        if matches!(node.kind, SyntaxKind::Binding | SyntaxKind::Assignment)
            && node.children.iter().any(|child| {
                child.kind == SyntaxKind::DeclarationQualifier
                    && node_text(&unit.source, child) == "constant"
            })
            && let Some(target) = first_write_to(package, unit, node.span, &unit.tree.root)
        {
            let name = node
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Name)
                .map_or("constant", |child| node_text(&unit.source, child));
            return Err(failure(
                &unit.source,
                "S2022",
                format!("constant binding `{name}` cannot be reassigned"),
                target.span,
            ));
        }
        if matches!(node.kind, SyntaxKind::Binding | SyntaxKind::Assignment)
            && node.children.iter().any(|child| {
                child.kind == SyntaxKind::DeclarationQualifier
                    && node_text(&unit.source, child) == "global"
            })
            && let Some(target) = node
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Name)
            && let Some(symbol) =
                package.resolve_name_at(unit, target.span.start, node_text(&unit.source, target))
            && symbol
                .declaration_span
                .is_some_and(|span| declaration_is_constant(package, span))
        {
            return Err(failure(
                &unit.source,
                "S2022",
                format!(
                    "constant binding `{}` cannot be reassigned",
                    node_text(&unit.source, target)
                ),
                target.span,
            ));
        }
        for child in &node.children {
            visit_declarations(package, unit, child)?;
        }
        Ok(())
    }

    for unit in &package.units {
        visit_declarations(package, unit, &unit.tree.root)?;
    }
    Ok(())
}

fn declaration_is_constant_in_unit(unit: &SemanticUnit, span: Span) -> bool {
    fn find(node: &SyntaxNode, span: Span, source: &SourceFile) -> Option<bool> {
        if node.span == span {
            return Some(node.children.iter().any(|child| {
                child.kind == SyntaxKind::DeclarationQualifier
                    && node_text(source, child) == "constant"
            }));
        }
        node.children
            .iter()
            .find_map(|child| find(child, span, source))
    }

    span.file == unit.source.id() && find(&unit.tree.root, span, &unit.source).unwrap_or(false)
}

fn declaration_is_constant(package: &SemanticPackage, span: Span) -> bool {
    package
        .units
        .iter()
        .find(|unit| unit.source.id() == span.file)
        .is_some_and(|unit| declaration_is_constant_in_unit(unit, span))
}

#[expect(
    clippy::too_many_lines,
    reason = "the global assignment transfer rules remain visible as one analysis"
)]
fn validate_global_definite_assignment(package: &SemanticPackage) -> Result<(), SemanticFailure> {
    fn has_qualifier(unit: &SemanticUnit, node: &SyntaxNode, qualifier: &str) -> bool {
        node.children.iter().any(|child| {
            child.kind == SyntaxKind::DeclarationQualifier
                && node_text(&unit.source, child) == qualifier
        })
    }

    fn has_initializer(unit: &SemanticUnit, node: &SyntaxNode) -> bool {
        unit.source.text()[node.span.start..node.span.end].contains('=')
    }

    fn global_name<'a>(unit: &'a SemanticUnit, node: &'a SyntaxNode) -> Option<&'a str> {
        node.children
            .iter()
            .find(|child| child.kind == SyntaxKind::Name)
            .map(|child| node_text(&unit.source, child))
    }

    fn collect_writes(
        package: &SemanticPackage,
        unit: &SemanticUnit,
        node: &SyntaxNode,
        writes: &mut BTreeSet<String>,
    ) {
        if matches!(node.kind, SyntaxKind::Binding | SyntaxKind::Assignment)
            && has_qualifier(unit, node, "global")
            && has_initializer(unit, node)
            && let Some(name) = global_name(unit, node)
        {
            writes.insert(name.to_owned());
        } else if node.kind == SyntaxKind::PostfixExpression
            && let Some(target) = node.children.first()
            && package
                .resolve_name_at(unit, target.span.start, node_text(&unit.source, target))
                .is_some_and(|symbol| symbol.global)
        {
            writes.insert(node_text(&unit.source, target).to_owned());
        }
        for child in &node.children {
            collect_writes(package, unit, child, writes);
        }
    }

    fn validate_node(
        package: &SemanticPackage,
        unit: &SemanticUnit,
        node: &SyntaxNode,
        relevant: &BTreeSet<String>,
        assigned: &mut BTreeSet<String>,
    ) -> Result<(), SemanticFailure> {
        if matches!(node.kind, SyntaxKind::Binding | SyntaxKind::Assignment)
            && has_qualifier(unit, node, "global")
        {
            let name_node = node
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Name);
            for child in &node.children {
                if Some(child.span) != name_node.map(|name| name.span) {
                    validate_node(package, unit, child, relevant, assigned)?;
                }
            }
            if has_initializer(unit, node)
                && let Some(name) = name_node.map(|name| node_text(&unit.source, name))
            {
                assigned.insert(name.to_owned());
            }
            return Ok(());
        }
        if node.kind == SyntaxKind::PostfixExpression
            && let Some(target) = node.children.first()
            && let Some(symbol) =
                package.resolve_name_at(unit, target.span.start, node_text(&unit.source, target))
            && symbol.global
        {
            let name = node_text(&unit.source, target);
            if relevant.contains(name) && !assigned.contains(name) {
                return Err(failure(
                    &unit.source,
                    "T0007",
                    format!("`{name}` may be read before it is assigned"),
                    target.span,
                ));
            }
            assigned.insert(name.to_owned());
            return Ok(());
        }
        if node.kind == SyntaxKind::Name
            && let Some(symbol) =
                package.resolve_name_at(unit, node.span.start, node_text(&unit.source, node))
            && symbol.global
        {
            let name = node_text(&unit.source, node);
            if relevant.contains(name) && !assigned.contains(name) {
                return Err(failure(
                    &unit.source,
                    "T0007",
                    format!("`{name}` may be read before it is assigned"),
                    node.span,
                ));
            }
            return Ok(());
        }
        if node.kind == SyntaxKind::IfStatement {
            if let Some(condition) = node.children.first() {
                validate_node(package, unit, condition, relevant, assigned)?;
            }
            let incoming = assigned.clone();
            let mut branch_results = Vec::new();
            for branch in node.children.iter().skip(1) {
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
                    validate_node(package, unit, branch_block, relevant, &mut branch_assigned)?;
                    branch_results.push(branch_assigned);
                }
            }
            if !node
                .children
                .iter()
                .any(|child| child.kind == SyntaxKind::ElseClause)
            {
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
            return Ok(());
        }
        if node.kind == SyntaxKind::WhileStatement {
            let before = assigned.clone();
            for child in &node.children {
                let mut branch = before.clone();
                validate_node(package, unit, child, relevant, &mut branch)?;
            }
            return Ok(());
        }
        for child in &node.children {
            validate_node(package, unit, child, relevant, assigned)?;
        }
        Ok(())
    }

    let mut uninitialized = package
        .globals
        .values()
        .filter(|symbol| symbol.kind == SymbolKind::Binding)
        .map(|symbol| symbol.name.clone())
        .collect::<BTreeSet<_>>();
    for unit in &package.units {
        for node in &unit.tree.root.children {
            if matches!(node.kind, SyntaxKind::Binding | SyntaxKind::Assignment)
                && has_qualifier(unit, node, "global")
                && has_initializer(unit, node)
                && let Some(name) = global_name(unit, node)
            {
                uninitialized.remove(name);
            }
        }
    }
    if uninitialized.is_empty() {
        return Ok(());
    }

    let mut writes = BTreeSet::new();
    for unit in &package.units {
        collect_writes(package, unit, &unit.tree.root, &mut writes);
    }
    for unit in &package.units {
        for function in unit
            .tree
            .root
            .children
            .iter()
            .filter(|node| node.kind == SyntaxKind::FunctionDeclaration)
        {
            let mut function_writes = BTreeSet::new();
            collect_writes(package, unit, function, &mut function_writes);
            let relevant = uninitialized
                .iter()
                .filter(|name| function_writes.contains(*name) || !writes.contains(*name))
                .cloned()
                .collect();
            validate_node(package, unit, function, &relevant, &mut BTreeSet::new())?;
        }
    }
    Ok(())
}

fn first_write_to<'a>(
    package: &SemanticPackage,
    unit: &SemanticUnit,
    declaration_span: Span,
    node: &'a SyntaxNode,
) -> Option<&'a SyntaxNode> {
    if matches!(
        node.kind,
        SyntaxKind::Assignment | SyntaxKind::PostfixExpression
    ) && node.span != declaration_span
        && let Some(target) = node.children.first()
        && target.kind == SyntaxKind::Name
        && package
            .resolve_name_at(unit, target.span.start, node_text(&unit.source, target))
            .is_some_and(|symbol| symbol.declaration_span == Some(declaration_span))
    {
        return Some(target);
    }
    node.children
        .iter()
        .find_map(|child| first_write_to(package, unit, declaration_span, child))
}

fn record_binding_mutability(package: &mut SemanticPackage) {
    let mutable_bindings = package
        .units
        .iter()
        .map(|unit| {
            unit.typed_bindings
                .iter()
                .map(|binding| {
                    let initially_assigned =
                        unit.source.text()[binding.span.start..binding.span.end].contains('=');
                    binding_span_is_mutated(package, unit, binding.span, initially_assigned)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mutable_parameters = package
        .units
        .iter()
        .map(|unit| {
            unit.functions
                .iter()
                .map(|function| {
                    function
                        .parameters
                        .iter()
                        .map(|parameter| {
                            binding_span_is_mutated(package, unit, parameter.span, true)
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    for ((unit, binding_mutability), parameter_mutability) in package
        .units
        .iter_mut()
        .zip(mutable_bindings)
        .zip(mutable_parameters)
    {
        for (binding, mutable) in unit.typed_bindings.iter_mut().zip(binding_mutability) {
            binding.mutable = mutable;
        }
        for (function, mutability) in unit.functions.iter_mut().zip(parameter_mutability) {
            for (parameter, mutable) in function.parameters.iter_mut().zip(mutability) {
                parameter.mutable = mutable;
            }
        }
    }
}

pub(crate) fn binding_span_is_mutated(
    package: &SemanticPackage,
    unit: &SemanticUnit,
    declaration_span: Span,
    initially_assigned: bool,
) -> bool {
    fn writes(
        package: &SemanticPackage,
        unit: &SemanticUnit,
        declaration_span: Span,
        node: &SyntaxNode,
    ) -> usize {
        let writes_here = usize::from(
            matches!(
                node.kind,
                SyntaxKind::Assignment | SyntaxKind::PostfixExpression
            ) && node.span != declaration_span
                && node.children.first().is_some_and(|target| {
                    target.kind == SyntaxKind::Name
                        && !package.is_lexical_replacement(
                            unit,
                            node.span,
                            node_text(&unit.source, target),
                        )
                        && package
                            .resolve_name_at(
                                unit,
                                target.span.start,
                                node_text(&unit.source, target),
                            )
                            .is_some_and(|symbol| symbol.declaration_span == Some(declaration_span))
                }),
        );
        writes_here
            + node
                .children
                .iter()
                .map(|child| writes(package, unit, declaration_span, child))
                .sum::<usize>()
    }

    writes(package, unit, declaration_span, &unit.tree.root) > usize::from(!initially_assigned)
}

fn namespace_with_objects<'a>(
    path: &str,
    names: impl IntoIterator<Item = &'a str>,
    kind: SymbolKind,
) -> Namespace {
    let symbols = names
        .into_iter()
        .map(|name| (name.to_owned(), compiler_owned_object(path, name, kind)))
        .collect();
    Namespace { symbols }
}

fn compiler_owned_object(path: &str, name: &str, kind: SymbolKind) -> Symbol {
    Symbol {
        identity: format!("{path}::{name}"),
        name: name.to_owned(),
        namespace: path.to_owned(),
        visibility: Visibility::Public,
        global: false,
        constant: false,
        kind,
        declaration_span: None,
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
        .find(|child| child.kind == SyntaxKind::Name)
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
