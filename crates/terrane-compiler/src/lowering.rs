use std::fmt::Write as _;

use num_bigint::BigInt;

use crate::{
    ScalarType, SourceFile, TypeCategory,
    rust_ir::{Item, Module, Program},
    semantics::{
        ArithmeticFamily, CoercionPolicy, ContextualConstant, ElementType, FunctionContract,
        MemberFamily, SemanticPackage, SemanticUnit, StringFamily, SymbolKind, TypedBinding,
        ValueType, binding_span_is_mutated, binding_span_is_read, binding_store_value_is_read,
        bound_method, contextual_constant, narrowed_optional_type, narrowed_value_type,
        promoted_integer_type, string_call_selection,
    },
    syntax::{SyntaxKind, SyntaxNode},
};

pub(crate) fn lower(package: &SemanticPackage) -> Program {
    let mut globals = String::new();
    if package_uses_structured_errors(package) {
        emit_error_support(&mut globals);
    }
    emit_global_storage(package, &mut globals);
    let modules = package
        .units
        .iter()
        .map(|unit| {
            let mut emitter = Emitter {
                package,
                unit,
                source: &unit.source,
                output: String::new(),
                indent: 0,
                continue_label: None,
                loop_counter: 0,
                return_type: None,
                parameter_types: Vec::new(),
                namespace_initializer: None,
                propagate_errors: false,
                function_errors: false,
                try_counter: 0,
                current_error: None,
                current_function: None,
                try_completion: false,
                in_loop: false,
            };
            emitter.emit_union_types();
            let mut items = Vec::new();
            if !emitter.output.is_empty() {
                items.push(Item::generated(&emitter.output));
                emitter.output.clear();
            }
            for node in &unit.tree.root.children {
                match node.kind {
                    SyntaxKind::Binding | SyntaxKind::Assignment => {
                        emitter.namespace_binding(node);
                    }
                    SyntaxKind::FunctionDeclaration => emitter.function(node),
                    _ => {}
                }
                if !emitter.output.is_empty() {
                    items.push(Item::sourced(node.span, &emitter.output));
                    emitter.output.clear();
                }
            }
            Module {
                source_path: display_path(unit.source.path()),
                namespace: unit.namespace.clone(),
                items,
            }
        })
        .collect();
    Program {
        version: crate::VERSION,
        globals: (!globals.is_empty())
            .then(|| Item::generated(&globals))
            .into_iter()
            .collect(),
        modules,
    }
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "these independent lexical control contexts are saved and restored separately"
)]
struct Emitter<'a> {
    package: &'a SemanticPackage,
    unit: &'a SemanticUnit,
    source: &'a SourceFile,
    output: String,
    indent: usize,
    continue_label: Option<String>,
    loop_counter: usize,
    return_type: Option<ValueType>,
    parameter_types: Vec<(String, ValueType)>,
    namespace_initializer: Option<(String, String)>,
    propagate_errors: bool,
    function_errors: bool,
    try_counter: usize,
    current_error: Option<String>,
    current_function: Option<String>,
    try_completion: bool,
    in_loop: bool,
}

fn package_uses_structured_errors(package: &SemanticPackage) -> bool {
    fn contains(package: &SemanticPackage, unit: &SemanticUnit, node: &SyntaxNode) -> bool {
        matches!(
            node.kind,
            SyntaxKind::ThrowStatement | SyntaxKind::TryStatement | SyntaxKind::IndexExpression
        ) || string_call_selection(&unit.source, node)
            .is_some_and(|selection| selection.family == StringFamily::Decode)
            || node
                .children
                .iter()
                .any(|child| contains(package, unit, child))
            || (node.kind == SyntaxKind::CallExpression
                && node.children.first().is_some_and(|callee| {
                    let range = if callee.kind == SyntaxKind::MemberExpression {
                        callee.children.first()
                    } else {
                        Some(callee)
                    };
                    range.is_some_and(|range| {
                        package
                            .resolve_name_at(
                                unit,
                                range.span.start,
                                &unit.source.text()[range.span.start..range.span.end],
                            )
                            .is_some_and(|symbol| symbol.identity == "/core/collections::range")
                    })
                }))
            || (node.kind == SyntaxKind::CallExpression
                && node.children.first().is_some_and(|callee| {
                    callee.kind == SyntaxKind::MemberExpression
                        && callee.children.get(1).is_some_and(|member| {
                            &unit.source.text()[member.span.start..member.span.end] == "set"
                        })
                }))
    }
    package.units.iter().any(|unit| {
        unit.functions.iter().any(|contract| contract.throws)
            || contains(package, unit, &unit.tree.root)
    })
}

fn emit_error_support(output: &mut String) {
    output.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n\
         enum TerraneErrorKind {\n\
         ArithmeticOverflow,\nDivisionByZero,\nIntegerConversionOverflow,\nNegativeShiftCount,\n\
         CoercionError,\nDecodeError,\nIndexError,\nMissingKey,\nResourceError,\nSourceError,\n}\n\
         impl TerraneErrorKind {\n\
         fn from_source_name(name: &str) -> Self {\nmatch name {\n\
         \".arithmetic-overflow\" => Self::ArithmeticOverflow,\n\
         \".division-by-zero\" => Self::DivisionByZero,\n\
         \".integer-conversion-overflow\" => Self::IntegerConversionOverflow,\n\
         \".negative-shift-count\" => Self::NegativeShiftCount,\n\
         \".coercion-error\" => Self::CoercionError,\n\
         \".decode-error\" => Self::DecodeError,\n\
         \".index-error\" => Self::IndexError,\n\
         \".missing-key\" => Self::MissingKey,\n\
         \".resource-error\" => Self::ResourceError,\n\
         _ => Self::SourceError,\n}\n}\n\
         fn source_name(self) -> &'static str {\nmatch self {\n\
         Self::ArithmeticOverflow => \".arithmetic-overflow\",\n\
         Self::DivisionByZero => \".division-by-zero\",\n\
         Self::IntegerConversionOverflow => \".integer-conversion-overflow\",\n\
         Self::NegativeShiftCount => \".negative-shift-count\",\n\
         Self::CoercionError => \".coercion-error\",\n\
         Self::DecodeError => \".decode-error\",\n\
         Self::IndexError => \".index-error\",\n\
         Self::MissingKey => \".missing-key\",\n\
         Self::ResourceError => \".resource-error\",\n\
         Self::SourceError => \".error\",\n}\n}\n}\n\
         #[derive(Clone, Debug)]\nstruct TerraneError {\n\
         kind: TerraneErrorKind,\nmessage: String,\ncause: Option<Box<TerraneError>>,\n\
         context: Vec<&'static str>,\n}\n\
         impl TerraneError {\n\
         fn new(kind: TerraneErrorKind, message: impl Into<String>) -> Self {\n\
         Self { kind, message: message.into(), cause: None, context: Vec::new() }\n}\n",
    );
    output.push_str(
        "#[allow(dead_code)]\n\
         fn at(mut self, frame: &'static str) -> Self {\n\
         self.context.push(frame);\nself\n}\n",
    );
    output.push_str(
        "\
         fn render(&self) -> String {\n\
         let mut rendered = format!(\"{}: {}\", self.kind.source_name(), self.message);\n\
         if let Some(cause) = &self.cause {\nrendered.push_str(\"\\ncaused by: \");\nrendered.push_str(&cause.render());\n}\n\
         for frame in &self.context {\nrendered.push_str(\"\\nat \");\nrendered.push_str(frame);\n}\nrendered\n}\n}\n\
         impl std::fmt::Display for TerraneError {\n\
         fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n\
         formatter.write_str(&self.render())\n}\n}\n\
         impl From<terrane_int_support::ArithmeticError> for TerraneError {\n\
         fn from(error: terrane_int_support::ArithmeticError) -> Self {\n\
         Self::new(TerraneErrorKind::from_source_name(error.source_name()), error.to_string())\n}\n}\n\
         impl From<terrane_string_support::DecodeError> for TerraneError {\n\
         fn from(error: terrane_string_support::DecodeError) -> Self {\n\
         Self::new(TerraneErrorKind::DecodeError, error.to_string().trim_start_matches(\".decode-error: \"))\n}\n}\n\
         impl From<terrane_collection_support::IndexError> for TerraneError {\n\
         fn from(error: terrane_collection_support::IndexError) -> Self {\n\
         Self::new(TerraneErrorKind::IndexError, error.to_string())\n}\n}\n\
         impl From<terrane_collection_support::MissingKey> for TerraneError {\n\
         fn from(error: terrane_collection_support::MissingKey) -> Self {\n\
         Self::new(TerraneErrorKind::MissingKey, error.to_string())\n}\n}\n\
         impl From<terrane_collection_support::RangeStepError> for TerraneError {\n\
         fn from(error: terrane_collection_support::RangeStepError) -> Self {\n\
         Self::new(TerraneErrorKind::SourceError, error.to_string())\n}\n}\n\
         fn __terrane_uncaught(error: TerraneError) -> ! {\n\
         eprintln!(\"{}\", error.render());\nstd::process::exit(1);\n}\n\
         fn __terrane_generated_defect(message: &str) -> ! {\n\
         eprintln!(\"internal compiler defect: generated program reached an impossible completion: {message}\");\n\
         std::process::exit(5);\n}\n\
         #[allow(dead_code)]\nenum TerraneCompletion<T> {\nNormal,\nReturn(T),\nError(TerraneError),\nBreak,\nContinue,\n}\n",
    );
}

#[expect(
    clippy::too_many_lines,
    reason = "program-global declarations and their initialization policy remain auditable together"
)]
fn emit_global_storage(package: &SemanticPackage, output: &mut String) {
    for (name, symbol) in &package.globals {
        if symbol.kind != SymbolKind::Binding {
            continue;
        }
        let Some(span) = symbol.declaration_span else {
            continue;
        };
        let Some(unit) = package
            .units
            .iter()
            .find(|unit| unit.source.id() == span.file)
        else {
            continue;
        };
        let Some(node) = find_node_by_span(&unit.tree.root, span) else {
            continue;
        };
        let Some(name_node) = node
            .children
            .iter()
            .find(|child| child.kind == SyntaxKind::Name)
        else {
            continue;
        };
        let emitter = Emitter {
            package,
            unit,
            source: &unit.source,
            output: String::new(),
            indent: 0,
            continue_label: None,
            loop_counter: 0,
            return_type: None,
            parameter_types: Vec::new(),
            namespace_initializer: None,
            propagate_errors: false,
            function_errors: false,
            try_counter: 0,
            current_error: None,
            current_function: None,
            try_completion: false,
            in_loop: false,
        };
        let value_type = unit
            .typed_bindings
            .iter()
            .find(|binding| binding.span == span)
            .map(|binding| binding.value_type.clone())
            .or_else(|| emitter.value_type(name_node));
        let Some(ValueType::Scalar(scalar)) = value_type else {
            continue;
        };
        let initial = package.units.iter().rev().find_map(|candidate_unit| {
            candidate_unit
                .tree
                .root
                .children
                .iter()
                .rev()
                .find_map(|candidate| {
                    let global = candidate.children.iter().any(|child| {
                        child.kind == SyntaxKind::DeclarationQualifier
                            && candidate_unit.source.text()[child.span.start..child.span.end].trim()
                                == "global"
                    });
                    let candidate_name = candidate
                        .children
                        .iter()
                        .find(|child| child.kind == SyntaxKind::Name)?;
                    (global
                        && &candidate_unit.source.text()
                            [candidate_name.span.start..candidate_name.span.end]
                            == name.as_str())
                    .then_some((candidate_unit, candidate, candidate_name))
                })
        });
        let initial = initial.and_then(|(initial_unit, initial_node, initial_name)| {
            let name_index = initial_node
                .children
                .iter()
                .position(|child| child.span == initial_name.span)?;
            let initializer = binding_initializer(initial_node, name_index)?;
            let mut initial_emitter = Emitter {
                package,
                unit: initial_unit,
                source: &initial_unit.source,
                output: String::new(),
                indent: 0,
                continue_label: None,
                loop_counter: 0,
                return_type: None,
                parameter_types: Vec::new(),
                namespace_initializer: None,
                propagate_errors: false,
                function_errors: false,
                try_counter: 0,
                current_error: None,
                current_function: None,
                try_completion: false,
                in_loop: false,
            };
            Some(initial_emitter.expression_as(initializer, ValueType::Scalar(scalar)))
        });
        let initial = initial.map_or_else(|| "None".to_owned(), |value| format!("Some({value})"));
        writeln!(
            output,
            "static {}: std::sync::LazyLock<std::sync::Mutex<Option<{}>>> = std::sync::LazyLock::new(|| std::sync::Mutex::new({initial}));",
            global_binding_name(name),
            rust_type(scalar)
        )
        .unwrap();
    }
    if package
        .globals
        .values()
        .any(|symbol| symbol.kind == SymbolKind::Binding)
    {
        output.push_str(
            "fn __terrane_uninitialized_global(name: &str, path: &str, line: usize, column: usize) -> ! {\n    eprintln!(\"{path}:{line}:{column}: error[T0007]: `{name}` may be read before it is assigned\");\n    std::process::exit(1);\n}\n",
        );
    }
}

impl Emitter<'_> {
    fn global_storage(&self, node: &SyntaxNode) -> Option<String> {
        (node.kind == SyntaxKind::Name)
            .then(|| {
                self.package
                    .resolve_name_at(self.unit, node.span.start, self.text(node))
            })
            .flatten()
            .filter(|symbol| symbol.global && symbol.kind == SymbolKind::Binding)
            .map(|symbol| global_binding_name(&symbol.name))
    }

    fn global_assignment(&mut self, node: &SyntaxNode) -> bool {
        let Some(name) = node
            .children
            .iter()
            .find(|child| child.kind == SyntaxKind::Name)
        else {
            return false;
        };
        let declared_global = node.children.iter().any(|child| {
            child.kind == SyntaxKind::DeclarationQualifier && self.text(child) == "global"
        });
        let storage = if declared_global {
            Some(global_binding_name(self.text(name)))
        } else {
            self.global_storage(name)
        };
        let Some(storage) = storage else {
            return false;
        };
        let Some((name_index, _)) = node
            .children
            .iter()
            .enumerate()
            .find(|(_, child)| child.kind == SyntaxKind::Name)
        else {
            return false;
        };
        let Some(initializer) = binding_initializer(node, name_index) else {
            return false;
        };
        let value = if let Some(ty) = self.value_type(name) {
            self.expression_as(initializer, ty)
        } else {
            self.expression(initializer)
        };
        let value = Self::unwrapped_expression(value);
        self.line("{");
        self.indent += 1;
        self.line(&format!("let value = {value};"));
        self.line(&format!(
            "*{storage}.lock().expect(\"program-global lock poisoned\") = Some(value);"
        ));
        self.indent -= 1;
        self.line("}");
        true
    }
    #[expect(
        clippy::too_many_lines,
        reason = "namespace initialization sequencing remains auditable as one lowering operation"
    )]
    fn namespace_binding(&mut self, node: &SyntaxNode) {
        if node.children.iter().any(|child| {
            child.kind == SyntaxKind::DeclarationQualifier && self.text(child) == "global"
        }) {
            return;
        }
        let Some(name_node) = node
            .children
            .iter()
            .find(|child| child.kind == SyntaxKind::Name)
        else {
            return;
        };
        let source_name = self.text(name_node);
        let Some(symbol) =
            self.package
                .resolve_name_at(self.unit, name_node.span.start, source_name)
        else {
            return;
        };
        let Some(declaration_span) = symbol.declaration_span else {
            return;
        };
        if symbol.global || !self.is_namespace_binding_span(declaration_span) {
            return;
        }
        let Some(binding) = self
            .unit
            .typed_bindings
            .iter()
            .find(|binding| binding.span == declaration_span)
        else {
            return;
        };
        let ValueType::Scalar(scalar) = binding.value_type.clone() else {
            return;
        };
        let initializers = self
            .unit
            .tree
            .root
            .children
            .iter()
            .filter(|candidate| {
                matches!(candidate.kind, SyntaxKind::Binding | SyntaxKind::Assignment)
                    && !candidate.children.iter().any(|child| {
                        child.kind == SyntaxKind::DeclarationQualifier
                            && self.text(child) == "global"
                    })
            })
            .filter_map(|candidate| {
                let (name_index, candidate_name) = candidate
                    .children
                    .iter()
                    .enumerate()
                    .find(|(_, child)| child.kind == SyntaxKind::Name)?;
                (self.text(candidate_name) == source_name)
                    .then_some(binding_initializer(candidate, name_index))
                    .flatten()
                    .cloned()
            })
            .collect::<Vec<_>>();
        let Some(first) = initializers.first() else {
            assert!(
                !self.text(node).contains('='),
                "analyzed initialized value binding must have a selected initializer"
            );
            return;
        };
        if !node.children.iter().any(|child| child.span == first.span) {
            return;
        }

        let ty = rust_type(scalar);
        let storage = namespace_binding_name(declaration_span.file, source_name);
        let local = format!("__terrane_{}_value", rust_name(source_name));
        self.namespace_initializer = Some((source_name.to_owned(), local.clone()));
        let values = initializers
            .iter()
            .map(|initializer| self.expression_as(initializer, binding.value_type.clone()))
            .collect::<Vec<_>>();
        self.namespace_initializer = None;
        if values.len() == 1 {
            self.line(&format!(
                "static {storage}: std::sync::LazyLock<{ty}> = std::sync::LazyLock::new(|| {});",
                values[0]
            ));
            return;
        }
        self.line(&format!(
            "static {storage}: std::sync::LazyLock<{ty}> = std::sync::LazyLock::new(|| {{"
        ));
        self.indent += 1;
        self.line(&format!("let mut {local} = {};", values[0]));
        for value in &values[1..] {
            self.line(&format!(
                "{local} = {};",
                Self::unwrapped_expression(value.clone())
            ));
        }
        self.line(&local);
        self.indent -= 1;
        self.line("});");
    }
    fn function(&mut self, node: &SyntaxNode) {
        let contract = self
            .unit
            .functions
            .iter()
            .find(|item| item.span == node.span)
            .expect("analyzed function declaration must have a semantic contract");
        self.line_start();
        let name = function_name(contract);
        write!(self.output, "fn {name}(").unwrap();
        for (index, parameter) in contract.parameters.iter().enumerate() {
            if index != 0 {
                self.output.push_str(", ");
            }
            let ty = parameter
                .value_type
                .clone()
                .map_or_else(|| "i128".to_owned(), rust_value_type);
            let mutable = if parameter.mutable { "mut " } else { "" };
            write!(self.output, "{mutable}{}: {ty}", rust_name(&parameter.name)).unwrap();
        }
        self.output.push(')');
        let function_errors = contract.throws && contract.name != "main";
        if function_errors {
            let result = contract
                .return_type
                .clone()
                .map_or_else(|| "()".to_owned(), rust_value_type);
            write!(self.output, " -> Result<{result}, TerraneError>").unwrap();
        } else if let Some(return_type) = contract.return_type.clone()
            && return_type != ValueType::Scalar(ScalarType::None)
        {
            write!(self.output, " -> {}", rust_value_type(return_type)).unwrap();
        }
        self.output.push_str(" {\n");
        let outer_return_type =
            std::mem::replace(&mut self.return_type, contract.return_type.clone());
        let outer_function_errors = std::mem::replace(&mut self.function_errors, function_errors);
        let outer_propagation = std::mem::replace(&mut self.propagate_errors, function_errors);
        let outer_function = self.current_function.replace(format!(
            "{}::{}",
            self.unit.namespace.trim_end_matches('/'),
            contract.name
        ));
        let outer_parameter_types = std::mem::replace(
            &mut self.parameter_types,
            contract
                .parameters
                .iter()
                .filter_map(|parameter| {
                    parameter
                        .value_type
                        .clone()
                        .map(|value_type| (parameter.name.clone(), value_type))
                })
                .collect(),
        );
        self.indent += 1;
        let block = node
            .children
            .iter()
            .find(|child| child.kind == SyntaxKind::Block);
        let unused_parameters = contract
            .parameters
            .iter()
            .filter(|parameter| {
                block.is_none_or(|block| {
                    !node_contains_name(&self.unit.source, block, &parameter.name)
                })
            })
            .map(|parameter| format!("&{}", rust_name(&parameter.name)))
            .collect::<Vec<_>>();
        match unused_parameters.as_slice() {
            [] => {}
            [parameter] => self.line(&format!("let _ = {parameter};")),
            parameters => self.line(&format!("let _ = ({});", parameters.join(", "))),
        }
        if let Some(block) = block {
            self.block(block);
        }
        if function_errors
            && contract
                .return_type
                .clone()
                .is_none_or(|ty| ty == ValueType::Scalar(ScalarType::None))
            && node
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Block)
                .is_some_and(block_may_fall_through)
        {
            self.line("Ok(())");
        }
        self.return_type = outer_return_type;
        self.function_errors = outer_function_errors;
        self.propagate_errors = outer_propagation;
        self.parameter_types = outer_parameter_types;
        self.current_function = outer_function;
        self.indent -= 1;
        self.line("}");
    }

    fn block(&mut self, block: &SyntaxNode) {
        for statement in &block.children {
            self.statement(statement);
        }
    }

    fn union_binding(&self, node: &SyntaxNode) -> Option<TypedBinding> {
        (node.kind == SyntaxKind::Name)
            .then(|| {
                self.unit
                    .typed_bindings
                    .iter()
                    .rev()
                    .find(|binding| {
                        binding.name == self.text(node)
                            && binding.span.start <= node.span.start
                            && !binding.destination_arms.is_empty()
                    })
                    .cloned()
            })
            .flatten()
    }

    fn union_value(&mut self, binding: &TypedBinding, value: &SyntaxNode) -> String {
        let actual = self
            .value_type(value)
            .and_then(|value_type| match value_type {
                ValueType::Scalar(scalar) => Some(scalar),
                _ => None,
            });
        let constant = binding
            .destination_arms
            .iter()
            .any(|arm| contextual_constant(self.source, value, *arm).is_some());
        let selected = (!constant)
            .then_some(actual)
            .flatten()
            .filter(|actual| binding.destination_arms.contains(actual))
            .or_else(|| {
                binding.destination_arms.iter().copied().find(|arm| {
                    contextual_constant(self.source, value, *arm)
                        .is_some_and(|result| result.is_ok())
                })
            })
            .or_else(|| {
                actual.and_then(|actual| {
                    is_numeric(actual).then(|| {
                        binding
                            .destination_arms
                            .iter()
                            .copied()
                            .find(|arm| is_numeric(*arm))
                            .expect("validated numeric union destination")
                    })
                })
            })
            .expect("validated union destination");
        let index = binding
            .destination_arms
            .iter()
            .position(|arm| *arm == selected)
            .expect("selected union arm belongs to destination");
        format!(
            "{}::Arm{index}({})",
            union_type_name(binding),
            self.expression_as(value, ValueType::Scalar(selected))
        )
    }

    fn emit_union_types(&mut self) {
        for binding in self
            .unit
            .typed_bindings
            .iter()
            .filter(|binding| !binding.destination_arms.is_empty())
        {
            let name = union_type_name(binding);
            self.line("#[allow(dead_code)]");
            self.line("#[derive(Clone)]");
            self.line(&format!("enum {name} {{"));
            self.indent += 1;
            for (index, arm) in binding.destination_arms.iter().enumerate() {
                self.line(&format!("Arm{index}({}),", rust_type(*arm)));
            }
            self.indent -= 1;
            self.line("}");
            self.line(&format!(
                "impl terrane_scalar_support::ScalarDisplay for {name} {{"
            ));
            self.indent += 1;
            self.line("fn write_scalar(&self, output: &mut String) {");
            self.indent += 1;
            self.line("match self {");
            self.indent += 1;
            for (index, _) in binding.destination_arms.iter().enumerate() {
                self.line(&format!(
                    "Self::Arm{index}(value) => terrane_scalar_support::ScalarDisplay::write_scalar(value, output),"
                ));
            }
            self.indent -= 1;
            self.line("}");
            self.indent -= 1;
            self.line("}");
            self.indent -= 1;
            self.line("}");
        }
    }

    fn statement(&mut self, node: &SyntaxNode) {
        match node.kind {
            SyntaxKind::Binding => {
                if !self.global_assignment(node) {
                    self.binding(node);
                }
            }
            SyntaxKind::Assignment => self.assignment(node),
            SyntaxKind::CallExpression => {
                let expression = self
                    .collection_mutation_statement(node)
                    .unwrap_or_else(|| self.expression(node));
                self.line(&format!("{expression};"));
            }
            SyntaxKind::PostfixExpression => self.postfix(node),
            SyntaxKind::IfStatement => self.if_statement(node),
            SyntaxKind::WhileStatement => self.while_statement(node),
            SyntaxKind::ForStatement => self.for_statement(node),
            SyntaxKind::ReturnStatement => {
                let value = node.children.first().map_or_else(
                    || "()".to_owned(),
                    |value| {
                        let value = if let Some(return_type) = self.return_type.clone() {
                            self.expression_as(value, return_type)
                        } else {
                            self.expression(value)
                        };
                        Self::unwrapped_expression(value)
                    },
                );
                if self.try_completion {
                    self.line(&format!("return TerraneCompletion::Return({value});"));
                } else if self.function_errors {
                    self.line(&format!("return Ok({value});"));
                } else if self.propagate_errors {
                    self.line(&format!("return Ok(Some({value}));"));
                } else {
                    self.line(&format!("return {value};"));
                }
            }
            SyntaxKind::ThrowStatement => self.throw_statement(node),
            SyntaxKind::TryStatement => self.try_statement(node),
            SyntaxKind::BreakStatement if self.try_completion => {
                self.line("return TerraneCompletion::Break;");
            }
            SyntaxKind::BreakStatement => self.line("break;"),
            SyntaxKind::ContinueStatement if self.try_completion => {
                self.line("return TerraneCompletion::Continue;");
            }
            SyntaxKind::ContinueStatement => {
                if let Some(label) = &self.continue_label {
                    self.line(&format!("break '{label};"));
                } else {
                    self.line("continue;");
                }
            }
            _ => {}
        }
    }

    fn collection_mutation_statement(&mut self, node: &SyntaxNode) -> Option<String> {
        let [callee, arguments] = node.children.as_slice() else {
            return None;
        };
        let [receiver, member] = callee.children.as_slice() else {
            return None;
        };
        if callee.kind != SyntaxKind::MemberExpression {
            return None;
        }
        let receiver_type = self.value_type(receiver)?;
        let receiver = self.expression(receiver);
        let values = arguments
            .children
            .iter()
            .map(|argument| argument.children.last().unwrap_or(argument))
            .collect::<Vec<_>>();
        match (receiver_type, self.text(member)) {
            (ValueType::List(item), "append") => Some(format!(
                "({receiver}).append({})",
                self.expression_as(values[0], item.value_type())
            )),
            (ValueType::List(item), "set") => {
                let index = self.expression_as(values[0], ValueType::Scalar(ScalarType::Int));
                let index = self.fallible(
                    format!("terrane_collection_support::index_from_int(&({index}))"),
                    node,
                );
                let value = self.expression_as(values[1], item.value_type());
                Some(self.fallible(format!("({receiver}).set({index}, {value})"), node))
            }
            (ValueType::Map(key, value) | ValueType::UnorderedMap(key, value), "set") => {
                Some(format!(
                    "({receiver}).set({}, {})",
                    self.expression_as(values[0], key.value_type()),
                    self.expression_as(values[1], value.value_type())
                ))
            }
            (ValueType::Set(item) | ValueType::UnorderedSet(item), "add") => Some(format!(
                "({receiver}).add({})",
                self.expression_as(values[0], item.value_type())
            )),
            _ => None,
        }
    }

    fn assignment(&mut self, node: &SyntaxNode) {
        if self.global_assignment(node) {
            return;
        }
        if let [target, value] = node.children.as_slice()
            && target.kind == SyntaxKind::IndexExpression
            && let [receiver, index] = target.children.as_slice()
            && let Some(receiver_type) = self.value_type(receiver)
        {
            let receiver_value = self.expression(receiver);
            match receiver_type {
                ValueType::List(item) => {
                    let index = self.expression_as(index, ValueType::Scalar(ScalarType::Int));
                    let index = self.fallible(
                        format!("terrane_collection_support::index_from_int(&({index}))"),
                        node,
                    );
                    let value = self.expression_as(value, item.value_type());
                    let mutation =
                        self.fallible(format!("({receiver_value}).set({index}, {value})"), node);
                    self.line(&format!("let _ = {mutation};"));
                }
                ValueType::Map(key, value_type) | ValueType::UnorderedMap(key, value_type) => {
                    let key = self.expression_as(index, key.value_type());
                    let value = self.expression_as(value, value_type.value_type());
                    self.line(&format!("let _ = ({receiver_value}).set({key}, {value});"));
                }
                _ => {}
            }
            return;
        }
        if self
            .unit
            .typed_bindings
            .iter()
            .any(|binding| binding.span == node.span)
        {
            self.binding(node);
            return;
        }
        let [left, right] = node.children.as_slice() else {
            return;
        };
        let assigned_binding = (left.kind == SyntaxKind::Name)
            .then(|| {
                self.package
                    .resolve_name_at(self.unit, left.span.start, self.text(left))
            })
            .flatten()
            .and_then(|symbol| symbol.declaration_span)
            .and_then(|span| {
                self.unit
                    .typed_bindings
                    .iter()
                    .find(|binding| binding.span == span)
            });
        let union_binding = self.union_binding(left);
        let value_type = assigned_binding
            .map(|binding| binding.value_type.clone())
            .or_else(|| self.value_type(left));
        let value = if let Some(binding) = union_binding {
            self.union_value(&binding, right)
        } else if let Some(value_type) = value_type {
            self.expression_as(right, value_type)
        } else {
            self.expression(right)
        };
        let value = Self::unwrapped_expression(value);
        let target = if left.kind == SyntaxKind::Name {
            rust_name(self.text(left))
        } else {
            self.expression(left)
        };
        self.line(&format!("{target} = {value};"));
        if let Some(binding) = assigned_binding
            && !binding_store_value_is_read(self.package, binding.span, node.span)
        {
            self.line(&format!("let _ = &mut {target};"));
        }
    }

    fn error_kind(&self, node: &SyntaxNode) -> String {
        self.package
            .resolve_name_at(self.unit, node.span.start, self.text(node))
            .and_then(|symbol| symbol.identity.strip_prefix("/core/errors::"))
            .unwrap_or_else(|| self.text(node).trim().trim_start_matches('.'))
            .to_owned()
    }

    fn throw_statement(&mut self, node: &SyntaxNode) {
        if let Some(current_error) = &self.current_error
            && node.children.is_empty()
        {
            if self.try_completion {
                self.line(&format!(
                    "return TerraneCompletion::Error({current_error}.clone());"
                ));
            } else if self.propagate_errors {
                self.line(&format!("return Err({current_error}.clone());"));
            } else {
                self.line(&format!("__terrane_uncaught({current_error}.clone());"));
            }
            return;
        }
        let Some(error_node) = node.children.first() else {
            return;
        };
        let name = self.error_kind(error_node);
        let context = self.error_context(node);
        let mut error = format!(
            "TerraneError::new(TerraneErrorKind::{}, \"{}\").at(\"{context}\")",
            rust_error_kind(&name),
            error_message(&name),
        );
        if let Some(current_error) = &self.current_error {
            error = format!(
                "{{ let mut error = {error}; error.cause = Some(Box::new({current_error}.clone())); error }}"
            );
        }
        if self.try_completion {
            self.line(&format!("return TerraneCompletion::Error({error});"));
        } else if self.propagate_errors {
            self.line(&format!("return Err({error});"));
        } else {
            self.line(&format!("__terrane_uncaught({error});"));
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the emitted try/catch/finally control flow is clearer as one auditable state machine"
    )]
    fn try_statement(&mut self, node: &SyntaxNode) {
        let Some(block) = node.children.first() else {
            return;
        };
        let index = self.try_counter;
        self.try_counter += 1;
        let result = self
            .return_type
            .clone()
            .map_or_else(|| "()".to_owned(), rust_value_type);
        let mutable = if node
            .children
            .iter()
            .any(|child| child.kind == SyntaxKind::FinallyClause)
        {
            "mut "
        } else {
            ""
        };
        self.line(&format!(
            "let {mutable}__terrane_completion_{index}: TerraneCompletion<{result}> = (|| {{"
        ));
        self.indent += 1;
        self.line(&format!(
            "let __terrane_try_{index}: TerraneCompletion<{result}> = (|| {{"
        ));
        self.indent += 1;
        let outer_completion = std::mem::replace(&mut self.try_completion, true);
        let outer_propagation = std::mem::replace(&mut self.propagate_errors, true);
        let outer_function_errors = std::mem::replace(&mut self.function_errors, false);
        self.block(block);
        if block_may_fall_through(block) {
            self.line("TerraneCompletion::Normal");
        }
        self.function_errors = outer_function_errors;
        self.propagate_errors = outer_propagation;
        self.indent -= 1;
        self.line("})();");
        self.line(&format!("match __terrane_try_{index} {{"));
        self.indent += 1;
        self.line("TerraneCompletion::Return(value) => return TerraneCompletion::Return(value),");
        self.line("TerraneCompletion::Break => return TerraneCompletion::Break,");
        self.line("TerraneCompletion::Continue => return TerraneCompletion::Continue,");
        self.line("TerraneCompletion::Normal => {}");
        self.line(&format!(
            "TerraneCompletion::Error(__terrane_error_{index}) => {{"
        ));
        self.indent += 1;
        self.line(&format!("let mut __terrane_handled_{index} = false;"));
        for clause in node
            .children
            .iter()
            .filter(|child| child.kind == SyntaxKind::CatchClause)
        {
            let descriptor = clause
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::Name);
            let condition = descriptor.map_or_else(
                || format!("!__terrane_handled_{index}"),
                |descriptor| {
                    let kind = self.error_kind(descriptor);
                    if kind == "error" {
                        format!("!__terrane_handled_{index}")
                    } else {
                        format!(
                            "!__terrane_handled_{index} && __terrane_error_{index}.kind == TerraneErrorKind::{}",
                            rust_error_kind(&kind)
                        )
                    }
                },
            );
            self.line(&format!("if {condition} {{"));
            self.indent += 1;
            self.line(&format!("__terrane_handled_{index} = true;"));
            let outer_error = self
                .current_error
                .replace(format!("__terrane_error_{index}"));
            if let Some(catch_block) = clause.children.last() {
                self.block(catch_block);
            }
            self.current_error = outer_error;
            self.indent -= 1;
            self.line("}");
        }
        self.try_completion = outer_completion;
        self.line(&format!("if !__terrane_handled_{index} {{"));
        self.indent += 1;
        self.line(&format!(
            "return TerraneCompletion::Error(__terrane_error_{index});"
        ));
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        self.line("TerraneCompletion::Normal");
        self.indent -= 1;
        self.line("})();");
        if let Some(finally) = node
            .children
            .iter()
            .find(|child| child.kind == SyntaxKind::FinallyClause)
            .and_then(|clause| clause.children.first())
        {
            self.line(&format!(
                "let __terrane_finally_{index}: TerraneCompletion<{result}> = (|| {{"
            ));
            self.indent += 1;
            let outer_completion = std::mem::replace(&mut self.try_completion, true);
            let outer_propagation = std::mem::replace(&mut self.propagate_errors, true);
            let outer_function_errors = std::mem::replace(&mut self.function_errors, false);
            self.block(finally);
            if block_may_fall_through(finally) {
                self.line("TerraneCompletion::Normal");
            }
            self.function_errors = outer_function_errors;
            self.propagate_errors = outer_propagation;
            self.try_completion = outer_completion;
            self.indent -= 1;
            self.line("})();");
            self.line(&format!("match __terrane_finally_{index} {{"));
            self.indent += 1;
            self.line("TerraneCompletion::Normal => {}");
            self.line(&format!(
                "replacement => __terrane_completion_{index} = replacement,"
            ));
            self.indent -= 1;
            self.line("}");
        }
        self.line(&format!("match __terrane_completion_{index} {{"));
        self.indent += 1;
        if statement_may_fall_through(node) {
            self.line("TerraneCompletion::Normal => {}");
        } else {
            self.line("TerraneCompletion::Normal => __terrane_generated_defect(\"non-fallthrough try completed normally\"),");
        }
        if self.try_completion {
            self.line(
                "TerraneCompletion::Return(value) => return TerraneCompletion::Return(value),",
            );
            self.line("TerraneCompletion::Error(error) => return TerraneCompletion::Error(error),");
        } else if self.function_errors {
            self.line("TerraneCompletion::Return(value) => return Ok(value),");
            self.line("TerraneCompletion::Error(error) => return Err(error),");
        } else if self.propagate_errors {
            self.line("TerraneCompletion::Return(value) => return Ok(Some(value)),");
            self.line("TerraneCompletion::Error(error) => return Err(error),");
        } else {
            self.line("TerraneCompletion::Return(value) => return value,");
            self.line("TerraneCompletion::Error(error) => __terrane_uncaught(error),");
        }
        if self.in_loop {
            self.line("TerraneCompletion::Break => break,");
            if let Some(label) = &self.continue_label {
                self.line(&format!("TerraneCompletion::Continue => break '{label},"));
            } else {
                self.line("TerraneCompletion::Continue => continue,");
            }
        } else {
            self.line("TerraneCompletion::Break | TerraneCompletion::Continue => __terrane_generated_defect(\"loop control escaped a non-loop try\"),");
        }
        self.indent -= 1;
        self.line("}");
    }

    fn binding(&mut self, node: &SyntaxNode) {
        let Some((name_index, name_node)) = node
            .children
            .iter()
            .enumerate()
            .find(|(_, child)| child.kind == SyntaxKind::Name)
        else {
            return;
        };
        let name = rust_name(self.text(name_node));
        let binding = self
            .unit
            .typed_bindings
            .iter()
            .find(|binding| binding.span == node.span);
        let storage_type = binding
            .and_then(|binding| binding.storage_type)
            .filter(|_| !binding_span_is_mutated(self.package, self.unit, node.span, true));
        let ty = binding.map(|binding| {
            if !binding.destination_arms.is_empty() {
                return union_type_name(binding);
            }
            if let Some(storage_type) = storage_type {
                return rust_type(storage_type).to_owned();
            }
            rust_value_type(binding.value_type.clone())
        });
        let initializer = binding_initializer(node, name_index);
        assert!(
            initializer.is_some() || !self.text(node).contains('='),
            "analyzed initialized value binding must have a selected initializer"
        );
        let mutable = binding.is_some_and(|binding| binding.mutable);
        if self
            .package
            .is_lexical_replacement(self.unit, node.span, self.text(name_node))
        {
            self.line(&format!("let _ = &{name};"));
        }
        self.line_start();
        self.output.push_str("let ");
        if mutable {
            self.output.push_str("mut ");
        }
        self.output.push_str(&name);
        if let Some(ty) = ty {
            write!(self.output, ": {ty}").unwrap();
        }
        if let Some(initializer) = initializer {
            let value = if let Some(binding) = binding
                && !binding.destination_arms.is_empty()
            {
                self.union_value(binding, initializer)
            } else if let Some(storage_type) = storage_type {
                self.expression_as(initializer, ValueType::Scalar(storage_type))
            } else if let Some(binding) = binding {
                self.expression_as(initializer, binding.value_type.clone())
            } else {
                self.expression(initializer)
            };
            let value = Self::unwrapped_expression(value);
            write!(self.output, " = {value}").unwrap();
        }
        self.output.push_str(";\n");
        if initializer.is_some() && !binding_store_value_is_read(self.package, node.span, node.span)
        {
            let borrow = if mutable { "&mut " } else { "&" };
            self.line(&format!("let _ = {borrow}{name};"));
        }
    }

    fn postfix(&mut self, node: &SyntaxNode) {
        let Some(value) = node.children.first() else {
            return;
        };
        let operator = &self.source.text()[value.span.end..node.span.end];
        let addition = operator.trim() == "++";
        let value_type = self.value_type(value);
        if let Some(storage) = self.global_storage(value) {
            self.line("{");
            self.indent += 1;
            self.line(&format!(
                "let mut value = {storage}.lock().expect(\"program-global lock poisoned\");"
            ));
            let failure = self.uninitialized_global_failure(value);
            let current = format!("value.clone().unwrap_or_else(|| {failure})");
            let updated = self.postfix_updated_value(&current, value_type, addition, node);
            self.line(&format!("*value = Some({updated});"));
            self.indent -= 1;
            self.line("}");
            return;
        }
        let target = self.expression(value);
        let updated = self.postfix_updated_value(&target, value_type, addition, node);
        self.line(&format!("{target} = {updated};"));
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "the optional recursive value type is matched as a complete lowering decision"
    )]
    fn postfix_updated_value(
        &self,
        value: &str,
        value_type: Option<ValueType>,
        addition: bool,
        node: &SyntaxNode,
    ) -> String {
        if value_type == Some(ValueType::Scalar(ScalarType::Int)) {
            let operator = if addition { "+" } else { "-" };
            return format!("{value}.clone() {operator} terrane_int_support::Int::from(1_i128)");
        }
        if matches!(value_type, Some(ValueType::Scalar(ty)) if ty.is_integer()) {
            let helper = if addition {
                "fixed_addition"
            } else {
                "fixed_subtraction"
            };
            return self.fallible(format!("terrane_int_support::{helper}({value}, 1)"), node);
        }
        unreachable!("semantic analysis validated postfix integer target");
    }

    fn if_statement(&mut self, node: &SyntaxNode) {
        let Some(condition) = node.children.first() else {
            return;
        };
        let Some(block) = node.children.get(1) else {
            return;
        };
        let condition = self.control_condition(condition);
        self.line(&format!("if {condition} {{"));
        self.indent += 1;
        self.block(block);
        self.indent -= 1;
        for clause in node.children.iter().skip(2) {
            self.line_start();
            if clause.children.len() == 1 {
                self.output.push_str("} else {\n");
                self.indent += 1;
                self.block(&clause.children[0]);
                self.indent -= 1;
            } else if let [condition, block] = clause.children.as_slice() {
                let condition = self.control_condition(condition);
                writeln!(self.output, "}} else if {condition} {{").unwrap();
                self.indent += 1;
                self.block(block);
                self.indent -= 1;
            }
        }
        self.line("}");
    }

    fn while_statement(&mut self, node: &SyntaxNode) {
        let [condition, block] = node.children.as_slice() else {
            return;
        };
        let condition = self.control_condition(condition);
        self.line(&format!("while {condition} {{"));
        self.indent += 1;
        let outer_continue = self.continue_label.take();
        let outer_loop = std::mem::replace(&mut self.in_loop, true);
        self.block(block);
        self.in_loop = outer_loop;
        self.continue_label = outer_continue;
        self.indent -= 1;
        self.line("}");
    }

    fn for_statement(&mut self, node: &SyntaxNode) {
        match node.children.as_slice() {
            [target, collection, block] if target.kind == SyntaxKind::ForTarget => {
                let Some(name) = target.children.first() else {
                    return;
                };
                let name_span = name.span;
                let mutable = if binding_span_is_mutated(self.package, self.unit, name.span, true) {
                    "mut "
                } else {
                    ""
                };
                let name = rust_name(self.text(name));
                let collection_type = self.value_type(collection);
                let collection = self.expression(collection);
                let iterator = format!("__terrane_iterator_{}", self.loop_counter);
                self.loop_counter += 1;
                let constructor = match collection_type {
                    Some(ValueType::Scalar(ScalarType::Bytes)) => {
                        format!("terrane_collection_support::bytes_iterator(&({collection}))")
                    }
                    Some(ValueType::Iterator(_)) => format!("&mut ({collection})"),
                    Some(
                        ValueType::List(_)
                        | ValueType::Map(_, _)
                        | ValueType::Set(_)
                        | ValueType::Tuple(_, _)
                        | ValueType::Range
                        | ValueType::UnorderedMap(_, _)
                        | ValueType::UnorderedSet(_),
                    ) => format!(
                        "terrane_collection_support::Iterable::terrane_iterator(&({collection}))"
                    ),
                    _ => format!("terrane_collection_support::string_iterator(&({collection}))"),
                };
                self.line(&format!("let mut {iterator} = {constructor};"));
                self.line("loop {");
                self.indent += 1;
                self.line(&format!("let {mutable}{name} = match {iterator}.next() {{"));
                self.indent += 1;
                self.line("terrane_collection_support::IterationStep::Item(item) => item,");
                self.line("terrane_collection_support::IterationStep::End => break,");
                self.indent -= 1;
                self.line("};");
                if !binding_span_is_read(self.package, name_span) {
                    self.line(&format!("let _ = &{name};"));
                }
                let outer_continue = self.continue_label.take();
                let outer_loop = std::mem::replace(&mut self.in_loop, true);
                self.block(block);
                self.in_loop = outer_loop;
                self.continue_label = outer_continue;
                self.indent -= 1;
                self.line("}");
            }
            [initial, condition, update, block] => {
                self.statement(initial);
                let condition = self.control_condition(condition);
                self.line(&format!("while {condition} {{"));
                self.indent += 1;
                let label = format!("__terrane_continue_{}", self.loop_counter);
                self.loop_counter += 1;
                self.line(&format!("'{label}: {{"));
                self.indent += 1;
                let outer_continue = self.continue_label.replace(label);
                let outer_loop = std::mem::replace(&mut self.in_loop, true);
                self.block(block);
                self.in_loop = outer_loop;
                self.continue_label = outer_continue;
                self.indent -= 1;
                self.line("}");
                self.statement(update);
                self.indent -= 1;
                self.line("}");
            }
            _ => {}
        }
    }

    fn expression(&mut self, node: &SyntaxNode) -> String {
        match node.kind {
            SyntaxKind::Literal => literal(self.text(node)),
            SyntaxKind::Name => self.name(node),
            SyntaxKind::GroupExpression => node
                .children
                .first()
                .map_or_else(String::new, |child| self.expression(child)),
            SyntaxKind::UnaryExpression => {
                let Some(operand) = node.children.last() else {
                    return String::new();
                };
                if self.is_adaptive_expression(operand) {
                    return self.adaptive_expression(node);
                }
                let operator = self.source.text()[node.span.start..operand.span.start].trim();
                let operator = match operator {
                    "not" => "!",
                    other => other,
                };
                format!("{operator}{}", self.expression(operand))
            }
            SyntaxKind::BinaryExpression => self.binary(node),
            SyntaxKind::TypeMembershipExpression => self.type_membership(node),
            SyntaxKind::MemberExpression => self.member(node),
            SyntaxKind::IndexExpression => self.index(node),
            SyntaxKind::CallExpression => self.call(node),
            SyntaxKind::PostfixExpression => node
                .children
                .first()
                .map_or_else(String::new, |child| self.expression(child)),
            _ => self.text(node).trim().to_owned(),
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "destination-directed lowering keeps every recursive value form in one auditable dispatch"
    )]
    fn expression_as(&mut self, node: &SyntaxNode, value_type: ValueType) -> String {
        if let ValueType::ScalarOrNone(scalar) = value_type {
            return if self.value_type(node) == Some(value_type) {
                self.expression(node)
            } else if self.text(node).trim() == "none"
                || self.value_type(node) == Some(ValueType::Scalar(ScalarType::None))
            {
                "None".to_owned()
            } else {
                format!(
                    "Some({})",
                    self.expression_as(node, ValueType::Scalar(scalar))
                )
            };
        }
        if let ValueType::Scalar(destination) = value_type
            && (node.kind != SyntaxKind::Literal
                || self.value_type(node) != Some(ValueType::Scalar(destination)))
            && let Some(Ok(constant)) = contextual_constant(self.source, node, destination)
        {
            return match constant {
                ContextualConstant::Integer(value) if destination == ScalarType::Int => {
                    adaptive_literal(&value.to_string())
                }
                ContextualConstant::Integer(value) => value.to_string(),
                ContextualConstant::Float32(value) => float32_literal(value),
                ContextualConstant::Float64(value) => float64_literal(value),
            };
        }
        if let ValueType::Scalar(destination) = value_type
            && let Some(ValueType::Scalar(source)) = self.value_type(node)
            && source != destination
            && is_numeric(source)
            && is_numeric(destination)
        {
            return self.numeric_destination(node, source, destination);
        }
        if let ValueType::Scalar(scalar) = value_type
            && scalar != ScalarType::Int
            && scalar.is_integer()
            && node.kind == SyntaxKind::UnaryExpression
            && let Some(operand) = node.children.last()
        {
            let operator = self.source.text()[node.span.start..operand.span.start].trim();
            return format!("{operator}{}", self.expression(operand));
        }
        match value_type {
            ValueType::Scalar(ScalarType::Int) => self.adaptive_expression(node),
            ValueType::Scalar(ScalarType::Float32)
                if self.value_type(node) == Some(ValueType::Scalar(ScalarType::Float64)) =>
            {
                format!("({}) as f32", self.expression(node))
            }
            ValueType::Scalar(ScalarType::String)
                if node.kind == SyntaxKind::Name
                    && self.lazy_namespace_binding_type(node).is_some() =>
            {
                format!("(*{}).clone()", self.namespace_name(node))
            }
            ValueType::List(item)
                if node.kind == SyntaxKind::Name
                    && self.is_builtin(node, "/core/collections::list") =>
            {
                format!(
                    "terrane_collection_support::List::<{}>::new(Vec::new())",
                    rust_element_type(item)
                )
            }
            ValueType::Tuple(item, _)
                if node.kind == SyntaxKind::Name
                    && self.is_builtin(node, "/core/collections::tuple") =>
            {
                format!(
                    "terrane_collection_support::Tuple::<{}>::new(Vec::new())",
                    rust_element_type(item)
                )
            }
            ValueType::Set(item)
                if node.kind == SyntaxKind::Name
                    && self.is_builtin(node, "/core/collections::set") =>
            {
                format!(
                    "terrane_collection_support::Set::<{}>::new(Vec::new())",
                    rust_element_type(item)
                )
            }
            ValueType::UnorderedSet(item)
                if node.kind == SyntaxKind::Name
                    && self.is_builtin(node, "/core/collections::unordered-set") =>
            {
                format!(
                    "terrane_collection_support::UnorderedSet::<{}>::new(Vec::new())",
                    rust_element_type(item)
                )
            }
            ValueType::Map(key, value)
                if node.kind == SyntaxKind::Name
                    && self.is_builtin(node, "/core/collections::map") =>
            {
                format!(
                    "terrane_collection_support::Map::<{}, {}>::new(Vec::new())",
                    rust_element_type(key),
                    rust_element_type(value)
                )
            }
            ValueType::UnorderedMap(key, value)
                if node.kind == SyntaxKind::Name
                    && self.is_builtin(node, "/core/collections::unordered-map") =>
            {
                format!(
                    "terrane_collection_support::UnorderedMap::<{}, {}>::new(Vec::new())",
                    rust_element_type(key),
                    rust_element_type(value)
                )
            }
            ValueType::List(_)
            | ValueType::Map(_, _)
            | ValueType::Set(_)
            | ValueType::Tuple(_, _)
            | ValueType::Range
            | ValueType::Entry(_, _)
            | ValueType::UnorderedMap(_, _)
            | ValueType::UnorderedSet(_)
                if node.kind == SyntaxKind::Name =>
            {
                format!("({}).clone()", self.expression(node))
            }
            _ => self.expression(node),
        }
    }

    fn adaptive_expression(&mut self, node: &SyntaxNode) -> String {
        match node.kind {
            SyntaxKind::Literal => adaptive_literal(self.text(node)),
            SyntaxKind::Name if self.lazy_namespace_binding_type(node).is_some() => {
                format!("(*{}).clone()", self.namespace_name(node))
            }
            SyntaxKind::Name if self.small_int_binding(node).is_some() => {
                format!(
                    "terrane_int_support::Int::from(({}) as i128)",
                    self.name(node)
                )
            }
            SyntaxKind::Name => format!("{}.clone()", self.name(node)),
            SyntaxKind::GroupExpression => {
                node.children.first().map_or_else(String::new, |child| {
                    format!("({})", self.adaptive_expression(child))
                })
            }
            SyntaxKind::UnaryExpression => {
                let Some(operand) = node.children.last() else {
                    return String::new();
                };
                let operator = self.source.text()[node.span.start..operand.span.start].trim();
                format!("{operator}{}", self.adaptive_expression(operand))
            }
            SyntaxKind::BinaryExpression => self.adaptive_binary(node),
            SyntaxKind::MemberExpression
                if node
                    .children
                    .get(1)
                    .is_some_and(|member| self.text(member) == "length") =>
            {
                format!("terrane_int_support::Int::from({})", self.expression(node))
            }
            _ => self.expression(node),
        }
    }

    fn adaptive_binary(&mut self, node: &SyntaxNode) -> String {
        let [left, right] = node.children.as_slice() else {
            return String::new();
        };
        let operator = self.source.text()[left.span.end..right.span.start].trim();
        let left = self.adaptive_expression(left);
        let right = self.adaptive_expression(right);
        match operator {
            "/" => {
                format!("terrane_int_support::unwrap_or_fail(({left}).euclidean_div(&({right})))")
            }
            "%" => format!("terrane_int_support::unwrap_or_fail(({left}).modulo(&({right})))"),
            _ => format!("({left} {operator} {right})"),
        }
    }

    fn adaptive_binary_as(&mut self, node: &SyntaxNode) -> String {
        let [left, right] = node.children.as_slice() else {
            return String::new();
        };
        let operator = self.source.text()[left.span.end..right.span.start].trim();
        let left = self.expression_as(left, ValueType::Scalar(ScalarType::Int));
        let right = self.expression_as(right, ValueType::Scalar(ScalarType::Int));
        match operator {
            "/" => {
                format!("terrane_int_support::unwrap_or_fail(({left}).euclidean_div(&({right})))")
            }
            "%" => format!("terrane_int_support::unwrap_or_fail(({left}).modulo(&({right})))"),
            _ => format!("({left} {operator} {right})"),
        }
    }

    fn is_adaptive_expression(&self, node: &SyntaxNode) -> bool {
        self.value_type(node) == Some(ValueType::Scalar(ScalarType::Int))
    }

    fn numeric_operation_type(&self, left: &SyntaxNode, right: &SyntaxNode) -> Option<ScalarType> {
        let scalar = |value_type| match value_type {
            ValueType::Scalar(scalar) => Some(scalar),
            _ => None,
        };
        let left_type = self.value_type(left).and_then(scalar);
        let right_type = self.value_type(right).and_then(scalar);
        if let Some(left_type) = left_type
            && is_numeric(left_type)
            && matches!(
                contextual_constant(self.source, right, left_type),
                Some(Ok(_))
            )
        {
            return Some(left_type);
        }
        if let Some(right_type) = right_type
            && is_numeric(right_type)
            && matches!(
                contextual_constant(self.source, left, right_type),
                Some(Ok(_))
            )
        {
            return Some(right_type);
        }
        match (left_type, right_type) {
            (Some(left), Some(right)) if left == right && is_numeric(left) => Some(left),
            (Some(left), Some(right)) if left.is_integer() && right.is_integer() => {
                Some(promoted_integer_type(left, right))
            }
            _ => None,
        }
    }

    fn optional_none_comparison(
        &mut self,
        left: &SyntaxNode,
        operator: &str,
        right: &SyntaxNode,
    ) -> Option<String> {
        if !matches!(operator, "==" | "!=") {
            return None;
        }
        let left_type = self.value_type(left);
        let right_type = self.value_type(right);
        let is_optional = |value_type| {
            matches!(
                value_type,
                Some(
                    ValueType::ScalarOrNone(_)
                        | ValueType::TextRangeOrNone
                        | ValueType::ElementOrNone(_)
                )
            )
        };
        let left_is_none = self.text(left).trim() == "none"
            || left_type == Some(ValueType::Scalar(ScalarType::None));
        let right_is_none = self.text(right).trim() == "none"
            || right_type == Some(ValueType::Scalar(ScalarType::None));
        if is_optional(left_type) && right_is_none {
            return Some(format!("({} {operator} None)", self.expression(left)));
        }
        if left_is_none && is_optional(right_type) {
            return Some(format!("(None {operator} {})", self.expression(right)));
        }
        None
    }

    fn binary(&mut self, node: &SyntaxNode) -> String {
        let [left, right] = node.children.as_slice() else {
            return String::new();
        };
        let source_operator = self.source.text()[left.span.end..right.span.start].trim();
        if source_operator == "is" {
            let result = matches!(
                (
                    self.descriptor_identity(left),
                    self.descriptor_identity(right),
                ),
                (Some(left), Some(right)) if left == right
            );
            let mut effects = Vec::new();
            if let Some(effect) = self.identity_operand_effect(left) {
                effects.push(effect);
            }
            if let Some(effect) = self.identity_operand_effect(right) {
                effects.push(effect);
            }
            return format!("{{ {} {result} }}", effects.join(" "));
        }
        if let Some(comparison) = self.optional_none_comparison(left, source_operator, right) {
            return comparison;
        }
        let comparison = matches!(source_operator, "==" | "!=" | "<" | "<=" | ">" | ">=");
        let left_is_small = self.small_int_binding(left).is_some()
            || matches!(
                contextual_constant(self.source, left, ScalarType::Int64),
                Some(Ok(_))
            );
        let right_is_small = self.small_int_binding(right).is_some()
            || matches!(
                contextual_constant(self.source, right, ScalarType::Int64),
                Some(Ok(_))
            );
        if comparison && left_is_small && right_is_small {
            let left = if self.small_int_binding(left).is_some() {
                self.expression(left)
            } else {
                self.expression_as(left, ValueType::Scalar(ScalarType::Int64))
            };
            let right = if self.small_int_binding(right).is_some() {
                self.expression(right)
            } else {
                self.expression_as(right, ValueType::Scalar(ScalarType::Int64))
            };
            return format!("({left} {source_operator} {right})");
        }
        if self.is_adaptive_expression(left)
            && matches!(source_operator, "==" | "!=" | "<" | "<=" | ">" | ">=")
        {
            return self.adaptive_binary(node);
        }
        if self.value_type(node) == Some(ValueType::Scalar(ScalarType::Int)) {
            return self.adaptive_binary_as(node);
        }
        if let Some(ValueType::Scalar(operation_type)) = self.value_type(node)
            && operation_type.is_integer()
            && operation_type != ScalarType::Int
            && let Some(operation) = match source_operator {
                "+" => Some("addition"),
                "-" => Some("subtraction"),
                "*" => Some("multiplication"),
                "/" => Some("division"),
                "%" => Some("remainder"),
                "<<" => Some("shift_left"),
                ">>" => Some("shift_right"),
                _ => None,
            }
        {
            let operation_type = ValueType::Scalar(operation_type);
            let left = Self::unwrapped_expression(self.expression_as(left, operation_type.clone()));
            let right = if matches!(source_operator, "<<" | ">>") {
                self.expression(right)
            } else {
                Self::unwrapped_expression(self.expression_as(right, operation_type))
            };
            let right = if matches!(source_operator, "<<" | ">>") {
                format!("&{right}")
            } else {
                right
            };
            let call = format!("terrane_int_support::fixed_{operation}({left}, {right})");
            return self.fallible(call, node);
        }
        if let Some(operation_type) = self.numeric_operation_type(left, right) {
            let left = self.expression_as(left, ValueType::Scalar(operation_type));
            let right = self.expression_as(right, ValueType::Scalar(operation_type));
            return format!("({left} {source_operator} {right})");
        }
        let operator = match source_operator {
            "and" => "&&",
            "or" => "||",
            other => other,
        };
        format!(
            "({} {operator} {})",
            self.expression(left),
            self.expression(right)
        )
    }

    fn type_membership(&mut self, node: &SyntaxNode) -> String {
        let [value, descriptor] = node.children.as_slice() else {
            return String::new();
        };
        let descriptor_type = self.descriptor_type(descriptor);
        let descriptor_category =
            crate::semantics::descriptor_expression_category(self.package, self.unit, descriptor);
        if let Some(binding) = self.union_binding(value)
            && let Some(category) = descriptor_category
        {
            let union_name = union_type_name(&binding);
            let expression = self.expression(value);
            let matching = binding
                .destination_arms
                .iter()
                .enumerate()
                .filter(|(_, arm)| arm.conforms_to(category))
                .map(|(index, _)| format!("{union_name}::Arm{index}(_)"))
                .collect::<Vec<_>>();
            return if matching.is_empty() {
                format!("{{ let _ = &{expression}; false }}")
            } else {
                format!("matches!(&{expression}, {})", matching.join(" | "))
            };
        }
        if let Some(binding) = self.union_binding(value)
            && let Some(descriptor) = descriptor_type
            && let Some(index) = binding
                .destination_arms
                .iter()
                .position(|arm| *arm == descriptor)
        {
            let union_name = union_type_name(&binding);
            let expression = self.expression(value);
            return format!("matches!(&{expression}, {union_name}::Arm{index}(_))");
        }
        let value_type = self.value_type(value);
        if let Some(category) = descriptor_category {
            return self.category_membership(value, value_type, category);
        }
        if let Some(destination) = descriptor_type
            && let Some(result) = contextual_constant(self.source, value, destination)
        {
            return result.is_ok().to_string();
        }
        if let Some(ValueType::ScalarOrNone(inner)) = value_type {
            let value = self.expression(value);
            return match descriptor_type {
                Some(ScalarType::None) => format!("({value}).is_none()"),
                Some(descriptor) if descriptor == inner => format!("({value}).is_some()"),
                _ => format!("{{ let _ = {value}; false }}"),
            };
        }
        let result = matches!(
            (value_type, descriptor_type),
            (Some(ValueType::Scalar(value)), Some(descriptor)) if value == descriptor
        );
        let effect = if value.kind == SyntaxKind::Name {
            let expression = Self::unwrapped_expression(self.expression(value));
            format!("let _ = &{expression};")
        } else {
            Self::discarded_expression(self.expression(value))
        };
        format!("{{ {effect} {result} }}")
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "category membership matches the optional recursive value type as one decision"
    )]
    fn category_membership(
        &mut self,
        node: &SyntaxNode,
        value_type: Option<ValueType>,
        category: TypeCategory,
    ) -> String {
        if let Some(ValueType::ScalarOrNone(inner)) = value_type {
            let value = self.expression(node);
            return if inner.conforms_to(category) {
                format!("({value}).is_some()")
            } else {
                format!("{{ let _ = {value}; false }}")
            };
        }
        let result = matches!(
            value_type,
            Some(ValueType::Scalar(value)) if value.conforms_to(category)
        );
        let effect = if node.kind == SyntaxKind::Name {
            let expression = Self::unwrapped_expression(self.expression(node));
            format!("let _ = &{expression};")
        } else {
            Self::discarded_expression(self.expression(node))
        };
        format!("{{ {effect} {result} }}")
    }

    fn identity_operand_effect(&mut self, node: &SyntaxNode) -> Option<String> {
        let effect = if node.kind == SyntaxKind::MemberExpression
            && node
                .children
                .get(1)
                .is_some_and(|member| self.text(member) == "type")
        {
            node.children.first()?
        } else {
            node
        };
        matches!(self.value_type(effect), Some(ValueType::Scalar(_)))
            .then(|| Self::discarded_expression(self.expression(effect)))
    }

    fn unwrapped_expression(mut expression: String) -> String {
        loop {
            let bytes = expression.as_bytes();
            if bytes.len() <= 2 || bytes.first() != Some(&b'(') || bytes.last() != Some(&b')') {
                break;
            }
            let mut depth = 0_usize;
            let wraps_expression = bytes.iter().enumerate().all(|(index, byte)| {
                match byte {
                    b'(' => depth += 1,
                    b')' => depth -= 1,
                    _ => {}
                }
                depth != 0 || index == bytes.len() - 1
            });
            if !wraps_expression {
                break;
            }
            expression = expression[1..expression.len() - 1].to_owned();
        }
        expression
    }

    fn discarded_expression(expression: String) -> String {
        format!("let _ = {};", Self::unwrapped_expression(expression))
    }

    fn value_type(&self, node: &SyntaxNode) -> Option<ValueType> {
        if let Some(value_type) = self.unit.inferred_value_type(node) {
            return Some(value_type);
        }
        match node.kind {
            SyntaxKind::Literal => match self.text(node).trim() {
                "true" | "false" => Some(ValueType::Scalar(ScalarType::Bool)),
                text if text.starts_with("b'") => Some(ValueType::Scalar(ScalarType::Bytes)),
                text if text.starts_with('\'') || text.starts_with('>') => {
                    Some(ValueType::Scalar(ScalarType::String))
                }
                text if text.contains('.') => Some(ValueType::Scalar(ScalarType::Float64)),
                text if text.chars().all(|character| {
                    character.is_ascii_hexdigit() || matches!(character, '_' | 'x' | 'o' | 'b')
                }) =>
                {
                    Some(ValueType::Scalar(ScalarType::Int))
                }
                _ => None,
            },
            SyntaxKind::Name => {
                let name = self.text(node).trim();
                self.unit
                    .typed_bindings
                    .iter()
                    .rev()
                    .find(|binding| binding.name == name && binding.span.start <= node.span.start)
                    .map(|binding| binding.value_type.clone())
                    .or_else(|| {
                        self.parameter_types
                            .iter()
                            .find(|(parameter, _)| parameter == name)
                            .map(|(_, value_type)| value_type.clone())
                    })
            }
            SyntaxKind::TypeExpression
            | SyntaxKind::GroupExpression
            | SyntaxKind::UnaryExpression => node
                .children
                .last()
                .and_then(|child| self.value_type(child)),
            _ => None,
        }
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "string-view dispatch matches the optional recursive receiver type as one decision"
    )]
    fn direct_string_view_length(
        &mut self,
        receiver: &SyntaxNode,
        receiver_type: Option<ValueType>,
        member: &SyntaxNode,
    ) -> Option<String> {
        if self.text(member) != "length" {
            return None;
        }
        let Some(ValueType::StringView(unit)) = receiver_type else {
            return None;
        };
        receiver.children.first().map(|source| {
            let source = self.expression(source);
            match unit {
                crate::semantics::TextUnit::Bytes => format!("({source}).len() as i128"),
                crate::semantics::TextUnit::Scalars => {
                    format!("({source}).chars().count() as i128")
                }
                crate::semantics::TextUnit::Graphemes => {
                    format!("terrane_string_support::length(&({source})) as i128")
                }
            }
        })
    }

    fn index(&mut self, node: &SyntaxNode) -> String {
        let [receiver, index] = node.children.as_slice() else {
            return String::new();
        };
        let receiver_type = self.value_type(receiver);
        let receiver = self.expression(receiver);
        match receiver_type {
            Some(ValueType::List(_) | ValueType::Tuple(_, _)) => {
                let index = if self.value_type(index) == Some(ValueType::Scalar(ScalarType::Int)) {
                    let index_value = self.expression_as(index, ValueType::Scalar(ScalarType::Int));
                    self.fallible(
                        format!("terrane_collection_support::index_from_int(&({index_value}))"),
                        node,
                    )
                } else {
                    format!("({}) as usize", self.expression(index))
                };
                self.fallible(format!("({receiver}).get_or_error({index})"), node)
            }
            Some(ValueType::Map(key, _) | ValueType::UnorderedMap(key, _)) => {
                let index_value = self.expression_as(index, key.value_type());
                self.fallible(format!("({receiver}).get_or_error(&({index_value}))"), node)
            }
            _ => String::new(),
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "member lowering keeps one ordered dispatch across scalar and collection surfaces"
    )]
    fn member(&mut self, node: &SyntaxNode) -> String {
        let [receiver, member] = node.children.as_slice() else {
            return String::new();
        };
        let receiver_type = self.value_type(receiver);
        if let Some(length) =
            self.direct_string_view_length(receiver, receiver_type.clone(), member)
        {
            return length;
        }
        let receiver = self.expression(receiver);
        match self.text(member) {
            "bytes" if receiver_type == Some(ValueType::Scalar(ScalarType::String)) => {
                format!("({receiver}).as_bytes().to_vec()")
            }
            "scalars" if receiver_type == Some(ValueType::Scalar(ScalarType::String)) => {
                format!("({receiver}).chars().map(|value| value.to_string()).collect::<Vec<_>>()")
            }
            "graphemes" if receiver_type == Some(ValueType::Scalar(ScalarType::String)) => {
                format!("terrane_string_support::graphemes(&({receiver})).collect::<Vec<_>>()")
            }
            "text" if receiver_type == Some(ValueType::TextRange) => {
                format!("({receiver}).text().to_owned()")
            }
            "bytes" | "scalars" | "graphemes" if receiver_type == Some(ValueType::TextRange) => {
                receiver
            }
            boundary @ ("start" | "end") => {
                let method = match (receiver_type.clone(), boundary) {
                    (
                        Some(ValueType::TextRangeView(crate::semantics::TextUnit::Bytes)),
                        "start",
                    ) => "byte_start",
                    (Some(ValueType::TextRangeView(crate::semantics::TextUnit::Bytes)), "end") => {
                        "byte_end"
                    }
                    (
                        Some(ValueType::TextRangeView(crate::semantics::TextUnit::Scalars)),
                        "start",
                    ) => "scalar_start",
                    (
                        Some(ValueType::TextRangeView(crate::semantics::TextUnit::Scalars)),
                        "end",
                    ) => "scalar_end",
                    (
                        Some(ValueType::TextRangeView(crate::semantics::TextUnit::Graphemes)),
                        "start",
                    ) => "grapheme_start",
                    (
                        Some(ValueType::TextRangeView(crate::semantics::TextUnit::Graphemes)),
                        "end",
                    ) => "grapheme_end",
                    _ => unreachable!("semantic analysis validated text-range boundary"),
                };
                format!("({receiver}).{method}() as i128")
            }
            "length" => match receiver_type {
                Some(
                    ValueType::StringView(crate::semantics::TextUnit::Bytes)
                    | ValueType::Scalar(ScalarType::Bytes),
                ) => {
                    format!("({receiver}).len() as i128")
                }
                Some(
                    ValueType::StringView(
                        crate::semantics::TextUnit::Scalars | crate::semantics::TextUnit::Graphemes,
                    )
                    | ValueType::StringList
                    | ValueType::TextRangeList,
                ) => format!("({receiver}).len() as i128"),
                Some(
                    ValueType::List(_)
                    | ValueType::Map(_, _)
                    | ValueType::Set(_)
                    | ValueType::Tuple(_, _)
                    | ValueType::UnorderedMap(_, _)
                    | ValueType::UnorderedSet(_),
                ) => format!("terrane_int_support::Int::from(({receiver}).length())"),
                _ => format!("terrane_string_support::length(&{receiver}) as i128"),
            },
            "key" if matches!(receiver_type, Some(ValueType::Entry(_, _))) => {
                format!("({receiver}).key.clone()")
            }
            "value" if matches!(receiver_type, Some(ValueType::Entry(_, _))) => {
                format!("({receiver}).value.clone()")
            }
            "type" => "()".to_owned(),
            mode @ ("round" | "floor" | "ceiling" | "truncate")
                if matches!(
                    receiver_type.clone(),
                    Some(ValueType::Scalar(ScalarType::Float32 | ScalarType::Float64))
                ) =>
            {
                let helper = if receiver_type == Some(ValueType::Scalar(ScalarType::Float32)) {
                    "rounded_f32"
                } else {
                    "rounded_f64"
                };
                let mode = match mode {
                    "round" => "TiesEven",
                    "floor" => "Floor",
                    "ceiling" => "Ceiling",
                    "truncate" => "Truncate",
                    _ => unreachable!(),
                };
                format!(
                    "terrane_int_support::unwrap_or_fail(terrane_int_support::{helper}({receiver}, terrane_int_support::FloatRounding::{mode}))"
                )
            }
            name => format!("{receiver}.{}", rust_name(name)),
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "all call forms share one ordering and error-propagation path"
    )]
    fn call(&mut self, node: &SyntaxNode) -> String {
        let [callee, arguments] = node.children.as_slice() else {
            return String::new();
        };
        if callee.kind == SyntaxKind::MemberExpression
            && let [family, child] = callee.children.as_slice()
            && self.text(child) == "checked"
            && family.kind == SyntaxKind::MemberExpression
            && let [receiver, member] = family.children.as_slice()
            && self.text(member) == "get"
            && let Some(receiver_type) = self.value_type(receiver)
            && let Some(argument) = arguments.children.first()
        {
            let argument = argument.children.last().unwrap_or(argument);
            let receiver_value = self.expression(receiver);
            return match receiver_type {
                ValueType::List(_) | ValueType::Tuple(_, _) => {
                    let index = self.expression_as(argument, ValueType::Scalar(ScalarType::Int));
                    format!(
                        "terrane_collection_support::index_from_int(&({index})).ok().and_then(|index| ({receiver_value}).get(index).cloned())"
                    )
                }
                ValueType::Map(key, _) | ValueType::UnorderedMap(key, _) => {
                    let key = self.expression_as(argument, key.value_type());
                    format!("({receiver_value}).get(&({key})).cloned()")
                }
                _ => String::new(),
            };
        }
        if let Some(string_call) = self.string_call(node, arguments) {
            return string_call;
        }
        if callee.kind == SyntaxKind::MemberExpression
            && let [receiver, member] = callee.children.as_slice()
            && let Some(receiver_type) = self.value_type(receiver)
        {
            let receiver_value = self.expression(receiver);
            let member_name = self.text(member).to_owned();
            let values = arguments
                .children
                .iter()
                .map(|argument| argument.children.last().unwrap_or(argument))
                .collect::<Vec<_>>();
            let call = match (receiver_type, member_name.as_str()) {
                (ValueType::List(item), "append") => Some(format!(
                    "({{ let collection = &mut ({receiver_value}); collection.append({}); collection.clone() }})",
                    self.expression_as(values[0], item.value_type())
                )),
                (ValueType::List(item), "set") => {
                    let index = self.expression_as(values[0], ValueType::Scalar(ScalarType::Int));
                    let index = self.fallible(
                        format!("terrane_collection_support::index_from_int(&({index}))"),
                        node,
                    );
                    let value = self.expression_as(values[1], item.value_type());
                    let mutation = self.fallible(format!("collection.set({index}, {value})"), node);
                    Some(format!(
                        "({{ let collection = &mut ({receiver_value}); {mutation}; collection.clone() }})"
                    ))
                }
                (ValueType::Map(key, value) | ValueType::UnorderedMap(key, value), "set") => {
                    Some(format!(
                        "({{ let collection = &mut ({receiver_value}); collection.set({}, {}); collection.clone() }})",
                        self.expression_as(values[0], key.value_type()),
                        self.expression_as(values[1], value.value_type())
                    ))
                }
                (ValueType::Set(item) | ValueType::UnorderedSet(item), "contains") => {
                    Some(format!(
                        "({receiver_value}).contains(&({}))",
                        self.expression_as(values[0], item.value_type())
                    ))
                }
                (ValueType::Set(item) | ValueType::UnorderedSet(item), "add") => Some(format!(
                    "({{ let collection = &mut ({receiver_value}); collection.add({}); collection.clone() }})",
                    self.expression_as(values[0], item.value_type())
                )),
                (ValueType::Set(item) | ValueType::UnorderedSet(item), "remove") => Some(format!(
                    "({receiver_value}).remove(&({}))",
                    self.expression_as(values[0], item.value_type())
                )),
                (
                    ValueType::Map(_, _) | ValueType::UnorderedMap(_, _),
                    "keys" | "values" | "entries",
                ) => Some(format!("({receiver_value}).{member_name}()")),
                _ => None,
            };
            if let Some(call) = call {
                return call;
            }
        }
        if let Some(method) = bound_method(self.source, callee) {
            let receiver_node = find_node_by_span(&self.unit.tree.root, method.receiver)
                .expect("validated bound method receiver");
            let receiver = self.expression(receiver_node);
            if method.family == MemberFamily::Coerce {
                return self.integer_coercion(&method, receiver_node, callee, arguments);
            }
            if let MemberFamily::Arithmetic(family) = method.family {
                return self.arithmetic_family(
                    family,
                    method.child,
                    receiver_node,
                    arguments,
                    node,
                );
            }
            let argument = arguments
                .children
                .first()
                .and_then(|argument| argument.children.last())
                .map(|value| self.expression(value))
                .unwrap_or_default();
            let callback_throws = method.family == MemberFamily::Parse
                && arguments
                    .children
                    .first()
                    .and_then(|argument| argument.children.last())
                    .and_then(|callback| self.contract_for_call(callback))
                    .is_some_and(|contract| contract.throws);
            let call = match method.family {
                MemberFamily::Parse => format!("{argument}({receiver})"),
                MemberFamily::Radix
                    if self.value_type(receiver_node)
                        == Some(ValueType::Scalar(ScalarType::String)) =>
                {
                    format!("terrane_int_support::parse_radix(&({receiver}), &({argument}))")
                }
                MemberFamily::Radix => {
                    format!("terrane_int_support::format_radix(&({receiver}), &({argument}))")
                }
                MemberFamily::Coerce | MemberFamily::Arithmetic(_) => unreachable!(),
            };
            return if method.family == MemberFamily::Parse && !callback_throws {
                if method.child == "checked" {
                    format!("Some({call})")
                } else {
                    call
                }
            } else if method.child == "checked" {
                format!("({call}).ok()")
            } else if method.family == MemberFamily::Parse {
                let context = self.error_context(node);
                if self.try_completion {
                    format!(
                        "match {call} {{ Ok(value) => value, Err(error) => return TerraneCompletion::Error(TerraneError::from(error).at(\"{context}\")) }}"
                    )
                } else if self.propagate_errors {
                    format!(
                        "({call}).map_err(|error| TerraneError::from(error).at(\"{context}\"))?"
                    )
                } else {
                    format!(
                        "({call}).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at(\"{context}\")))"
                    )
                }
            } else {
                self.fallible(call, node)
            };
        }
        if self.is_builtin(callee, "/core/collections::iterator") {
            let item_type = self
                .value_type(node)
                .and_then(|ty| match ty {
                    ValueType::Iterator(item) => Some(item),
                    _ => None,
                })
                .expect("validated iterator constructor has an item type");
            let values = arguments
                .children
                .iter()
                .map(|argument| argument.children.last().unwrap_or(argument))
                .map(|value| self.expression_as(value, item_type.value_type()))
                .collect::<Vec<_>>();
            return format!(
                "terrane_collection_support::Iterator::<{}>::new(vec![{}])",
                rust_element_type(item_type),
                values.join(", ")
            );
        }
        if let Some(value_type) = self.value_type(node) {
            let constructor = match value_type.clone() {
                ValueType::List(item) if self.is_builtin(callee, "/core/collections::list") => {
                    Some(("List", item))
                }
                ValueType::Tuple(item, _)
                    if self.is_builtin(callee, "/core/collections::tuple") =>
                {
                    Some(("Tuple", item))
                }
                ValueType::Set(item) if self.is_builtin(callee, "/core/collections::set") => {
                    Some(("Set", item))
                }
                ValueType::UnorderedSet(item)
                    if self.is_builtin(callee, "/core/collections::unordered-set") =>
                {
                    Some(("UnorderedSet", item))
                }
                _ => None,
            };
            if let Some((kind, item)) = constructor {
                let values = arguments
                    .children
                    .iter()
                    .map(|argument| argument.children.last().unwrap_or(argument))
                    .map(|value| self.expression_as(value, item.value_type()))
                    .collect::<Vec<_>>();
                return format!(
                    "terrane_collection_support::{kind}::<{}>::new(vec![{}])",
                    rust_element_type(item),
                    values.join(", ")
                );
            }
            if let ValueType::Entry(key, value) = value_type.clone()
                && self.is_builtin(callee, "/core/collections::entry")
            {
                let values = [
                    self.expression_as(
                        arguments.children[0]
                            .children
                            .last()
                            .unwrap_or(&arguments.children[0]),
                        key.value_type(),
                    ),
                    self.expression_as(
                        arguments.children[1]
                            .children
                            .last()
                            .unwrap_or(&arguments.children[1]),
                        value.value_type(),
                    ),
                ];
                return format!(
                    "terrane_collection_support::Entry::<{}, {}>::new({}, {})",
                    rust_element_type(key),
                    rust_element_type(value),
                    values[0],
                    values[1]
                );
            }
            if matches!(
                value_type,
                ValueType::Map(_, _) | ValueType::UnorderedMap(_, _)
            ) {
                let (kind, key, value) = match value_type.clone() {
                    ValueType::Map(key, value) => ("Map", key, value),
                    ValueType::UnorderedMap(key, value) => ("UnorderedMap", key, value),
                    _ => unreachable!(),
                };
                let identity = format!(
                    "/core/collections::{}",
                    if kind == "Map" {
                        "map"
                    } else {
                        "unordered-map"
                    }
                );
                if self.is_builtin(callee, &identity) {
                    let entries = arguments
                        .children
                        .iter()
                        .map(|argument| {
                            let value_node = argument.children.last().unwrap_or(argument);
                            if matches!(self.value_type(value_node), Some(ValueType::Entry(_, _))) {
                                return self.expression_as(
                                    value_node,
                                    ValueType::Entry(key.clone(), value.clone()),
                                );
                            }
                            let name = argument
                                .children
                                .first()
                                .map(|name| self.text(name).to_owned())
                                .expect("validated named map entry");
                            let value_expression =
                                self.expression_as(value_node, value.value_type());
                            format!(
                                "terrane_collection_support::Entry::new(String::from({name:?}), {value_expression})"
                            )
                        })
                        .collect::<Vec<_>>();
                    return format!(
                        "terrane_collection_support::{kind}::<{}, {}>::new(vec![{}])",
                        rust_element_type(key),
                        rust_element_type(value),
                        entries.join(", ")
                    );
                }
            }
            if value_type == ValueType::Range {
                let through = callee.kind == SyntaxKind::MemberExpression
                    && callee.children.first().is_some_and(|receiver| {
                        self.is_builtin(receiver, "/core/collections::range")
                    });
                if through || self.is_builtin(callee, "/core/collections::range") {
                    let mut values = arguments
                        .children
                        .iter()
                        .map(|argument| argument.children.last().unwrap_or(argument))
                        .map(|value| self.expression_as(value, ValueType::Scalar(ScalarType::Int)))
                        .collect::<Vec<_>>();
                    if values.len() == 2 {
                        values.push("terrane_int_support::Int::from(1_i64)".to_owned());
                    }
                    let method = if through { "through" } else { "new" };
                    return self.fallible(
                        format!(
                            "terrane_collection_support::Range::{method}({}, {}, {})",
                            values[0], values[1], values[2]
                        ),
                        node,
                    );
                }
            }
        }
        let mut values = arguments
            .children
            .iter()
            .map(|argument| argument.children.last().unwrap_or(argument))
            .map(|value| self.expression(value))
            .collect::<Vec<_>>();
        if self.is_builtin(callee, "/core/output::print") {
            if values.is_empty() {
                return "println!()".to_owned();
            }
            let values = values
                .into_iter()
                .map(|value| format!("terrane_scalar_support::scalar_text(&({value}))"))
                .collect::<Vec<_>>();
            let format = "{}".repeat(values.len());
            return format!("println!(\"{format}\", {})", values.join(", "));
        }
        if callee.kind == SyntaxKind::MemberExpression
            && callee
                .children
                .get(1)
                .is_some_and(|member| self.text(member) == "join")
        {
            let separator = self.expression(&callee.children[0]);
            let values = values
                .into_iter()
                .map(|value| format!("terrane_scalar_support::scalar_text(&({value}))"))
                .collect::<Vec<_>>();
            if values.is_empty() {
                return format!("{{ let _ = {separator}; String::new() }}");
            }
            return format!("vec![{}].join(&({separator}))", values.join(", "));
        }
        if callee.kind == SyntaxKind::MemberExpression
            && callee
                .children
                .get(1)
                .is_some_and(|member| self.text(member) == "concat")
        {
            let receiver = self.expression(&callee.children[0]);
            values.insert(0, receiver);
            let values = values
                .into_iter()
                .map(|value| format!("terrane_scalar_support::scalar_text(&({value}))"))
                .collect::<Vec<_>>();
            let format = "{}".repeat(values.len());
            return format!("format!(\"{format}\", {})", values.join(", "));
        }
        let contract = self.contract_for_call(callee).cloned();
        if let Some(contract) = &contract {
            let mut ordered = vec![None; contract.parameters.len()];
            let mut positional = 0;
            for argument in &arguments.children {
                let named = argument
                    .children
                    .first()
                    .filter(|child| child.kind == SyntaxKind::Name && argument.children.len() > 1);
                let index = named.map_or_else(
                    || {
                        let index = positional;
                        positional += 1;
                        index
                    },
                    |name| {
                        contract
                            .parameters
                            .iter()
                            .position(|parameter| parameter.name == self.text(name))
                            .expect("validated named argument")
                    },
                );
                let value = argument.children.last().unwrap_or(argument);
                let parameter = &contract.parameters[index];
                ordered[index] = Some(if let Some(ty) = parameter.value_type.clone() {
                    self.expression_as(value, ty)
                } else {
                    self.expression(value)
                });
            }
            self.append_defaults(contract, &mut ordered);
            values = ordered.into_iter().flatten().collect();
        }
        let name = contract
            .as_ref()
            .map_or_else(|| self.expression(callee), function_name);
        let call = format!("{name}({})", values.join(", "));
        if contract.is_some_and(|contract| contract.throws) {
            let context = self.error_context(node);
            if self.try_completion {
                format!(
                    "match {call} {{ Ok(value) => value, Err(error) => return TerraneCompletion::Error(error.at(\"{context}\")) }}"
                )
            } else if self.propagate_errors {
                format!("({call}).map_err(|error| error.at(\"{context}\"))?")
            } else {
                format!(
                    "({call}).unwrap_or_else(|error| __terrane_uncaught(error.at(\"{context}\")))"
                )
            }
        } else {
            call
        }
    }

    fn fallible(&self, call: impl AsRef<str>, node: &SyntaxNode) -> String {
        let call = call.as_ref();
        let context = self.error_context(node);
        if self.try_completion {
            format!(
                "match {call} {{ Ok(value) => value, Err(error) => return TerraneCompletion::Error(TerraneError::from(error).at(\"{context}\")) }}"
            )
        } else if self.propagate_errors {
            format!("({call}).map_err(|error| TerraneError::from(error).at(\"{context}\"))?")
        } else if package_uses_structured_errors(self.package) {
            format!(
                "({call}).unwrap_or_else(|error| __terrane_uncaught(TerraneError::from(error).at(\"{context}\")))"
            )
        } else {
            format!("terrane_int_support::unwrap_or_fail({call})")
        }
    }

    fn error_context(&self, node: &SyntaxNode) -> String {
        let (line, column) = self.source.line_column(node.span.start);
        let location = format!("{}:{line}:{column}", display_path(self.source.path()));
        self.current_function
            .as_ref()
            .map_or(location.clone(), |function| {
                format!("{function} ({location})")
            })
    }

    fn numeric_destination(
        &mut self,
        node: &SyntaxNode,
        source: ScalarType,
        destination: ScalarType,
    ) -> String {
        let value = self.expression(node);
        if destination == ScalarType::Int {
            if matches!(source, ScalarType::Float32 | ScalarType::Float64) {
                let helper = if source == ScalarType::Float32 {
                    "exact_int_f32"
                } else {
                    "exact_int_f64"
                };
                return self.fallible(format!("terrane_int_support::{helper}({value})"), node);
            }
            return if source == ScalarType::Uint128 {
                format!("terrane_int_support::adaptive(&({value}))")
            } else {
                format!("terrane_int_support::Int::from(({value}) as i128)")
            };
        }
        if source == ScalarType::Int && destination.is_integer() {
            return self.fallible(
                format!(
                    "terrane_int_support::coerce::<{}>(&({value}))",
                    rust_type(destination)
                ),
                node,
            );
        }
        if source == ScalarType::Int {
            let helper = if destination == ScalarType::Float32 {
                "exact_f32"
            } else {
                "exact_f64"
            };
            return self.fallible(format!("terrane_int_support::{helper}(&({value}))"), node);
        }
        if source.is_integer() && destination.is_integer() {
            if integer_range_contains(destination, source) {
                return format!("(({value}) as {})", rust_type(destination));
            }
            return format!(
                "{{ let source_value = {value}; terrane_int_support::unwrap_or_fail({}::try_from(source_value).map_err(|_| terrane_int_support::ArithmeticError::conversion_overflow(&source_value, \"{source}\", \"{destination}\", \"the value is outside the destination range\"))) }}",
                rust_type(destination)
            );
        }
        if source == ScalarType::Float32 && destination == ScalarType::Float64 {
            return format!("(({value}) as f64)");
        }
        if source.is_integer() {
            if exact_integer_float_widening(source, destination) {
                return format!("(({value}) as {})", rust_type(destination));
            }
            let helper = if destination == ScalarType::Float32 {
                "exact_f32"
            } else {
                "exact_f64"
            };
            return self.fallible(format!("terrane_int_support::{helper}(&({value}))"), node);
        }
        if destination.is_integer() {
            let helper = if source == ScalarType::Float32 {
                "exact_from_f32"
            } else {
                "exact_from_f64"
            };
            return self.fallible(
                format!(
                    "terrane_int_support::{helper}::<{}>({value})",
                    rust_type(destination)
                ),
                node,
            );
        }
        format!(
            "{{ let source_value = {value}; let converted = source_value as f32; if (converted as f64) == source_value {{ converted }} else {{ terrane_int_support::unwrap_or_fail(Err(terrane_int_support::ArithmeticError::conversion_overflow(&source_value, \"float64\", \"float32\", \"the floating value is not exactly representable\"))) }} }}"
        )
    }

    #[allow(clippy::too_many_lines)]
    fn string_call(&mut self, node: &SyntaxNode, arguments: &SyntaxNode) -> Option<String> {
        let selection = string_call_selection(self.source, node)?;
        let subject = find_node_by_span(&self.unit.tree.root, selection.receiver)
            .expect("selected string receiver belongs to this syntax tree");
        let family = selection.family.source_name();
        let child = selection.child.as_str();
        let receiver = self.expression(subject);
        let values = arguments
            .children
            .iter()
            .map(|argument| argument.children.last().unwrap_or(argument))
            .map(|value| self.expression(value))
            .collect::<Vec<_>>();
        let result = match (family, child) {
            ("trim", "default") => {
                format!("terrane_string_support::trim(&({receiver}))")
            }
            ("trim", mode @ ("start" | "end")) => {
                let helper = if mode == "start" {
                    "trim_start"
                } else {
                    "trim_end"
                };
                let pattern = values
                    .first()
                    .map_or_else(|| "None".to_owned(), |value| format!("Some(&({value}))"));
                format!("terrane_string_support::{helper}(&({receiver}), {pattern})")
            }
            ("contains", "default") => format!("({receiver}).contains(&({}))", values[0]),
            ("contains", "start") => format!("({receiver}).starts_with(&({}))", values[0]),
            ("contains", "end") => format!("({receiver}).ends_with(&({}))", values[0]),
            ("find", "default") => {
                format!(
                    "terrane_string_support::find(&({receiver}), &({}))",
                    values[0]
                )
            }
            ("find", "all") => {
                format!(
                    "terrane_string_support::find_all(&({receiver}), &({}))",
                    values[0]
                )
            }
            ("find", "count") => format!(
                "terrane_int_support::Int::from(terrane_string_support::find_all(&({receiver}), &({})).len() as i128)",
                values[0]
            ),
            ("upper", mode) => {
                let helper = match mode {
                    "default" => "upper",
                    "first" => "upper_first",
                    "words" => "upper_words",
                    _ => unreachable!(),
                };
                format!("terrane_string_support::{helper}(&({receiver}))")
            }
            ("lower", mode) => {
                let helper = match mode {
                    "default" => "lower",
                    "first" => "lower_first",
                    _ => unreachable!(),
                };
                format!("terrane_string_support::{helper}(&({receiver}))")
            }
            ("case-fold", _) => format!("terrane_string_support::case_fold(&({receiver}))"),
            ("normalise", form) => {
                format!("terrane_string_support::normalise(&({receiver}), {form:?})")
            }
            ("split", _) => {
                format!(
                    "terrane_string_support::split(&({receiver}), &({}))",
                    values[0]
                )
            }
            ("replace", _) => format!(
                "terrane_string_support::replace(&({receiver}), &({}), &({}))",
                values[0], values[1]
            ),
            ("encode", _) => {
                format!(
                    "terrane_string_support::encode(&({receiver}), {})",
                    values[0]
                )
            }
            ("decode", _) => self.fallible(
                format!(
                    "terrane_string_support::decode(&({receiver}), {})",
                    values[0]
                ),
                node,
            ),
            _ => unreachable!("semantic analysis validated string family"),
        };
        Some(result)
    }
    #[allow(clippy::too_many_lines)]
    fn arithmetic_family(
        &mut self,
        family: ArithmeticFamily,
        child: &str,
        receiver_node: &SyntaxNode,
        arguments: &SyntaxNode,
        call: &SyntaxNode,
    ) -> String {
        let Some(ValueType::Scalar(receiver_type)) = self.value_type(receiver_node) else {
            unreachable!("validated arithmetic receiver");
        };
        let receiver = if receiver_type == ScalarType::Int {
            self.adaptive_expression(receiver_node)
        } else {
            self.expression(receiver_node)
        };
        let argument = arguments
            .children
            .first()
            .and_then(|argument| argument.children.last())
            .map(|argument| {
                if receiver_type == ScalarType::Int {
                    self.adaptive_expression(argument)
                } else if matches!(
                    family,
                    ArithmeticFamily::ShiftLeft | ArithmeticFamily::ShiftRight
                ) {
                    self.expression(argument)
                } else {
                    self.expression_as(argument, ValueType::Scalar(receiver_type))
                }
            });
        if receiver_type == ScalarType::Int {
            let expression = match family {
                ArithmeticFamily::Add => format!("({receiver} + {})", argument.unwrap()),
                ArithmeticFamily::Subtract => format!("({receiver} - {})", argument.unwrap()),
                ArithmeticFamily::Multiply => format!("({receiver} * {})", argument.unwrap()),
                ArithmeticFamily::Divide => {
                    format!("({receiver}).euclidean_div(&({}))", argument.unwrap())
                }
                ArithmeticFamily::Remainder => {
                    format!("({receiver}).modulo(&({}))", argument.unwrap())
                }
                ArithmeticFamily::DivRem => {
                    format!("({receiver}).div_rem(&({}))", argument.unwrap())
                }
                ArithmeticFamily::Negate => format!("-({receiver})"),
                ArithmeticFamily::ShiftLeft => {
                    format!("({receiver}).shift_left(&({}))", argument.unwrap())
                }
                ArithmeticFamily::ShiftRight => {
                    format!("({receiver}).shift_right(&({}))", argument.unwrap())
                }
            };
            return if child == "checked" {
                format!("({expression}).ok()")
            } else if matches!(
                family,
                ArithmeticFamily::Divide
                    | ArithmeticFamily::Remainder
                    | ArithmeticFamily::DivRem
                    | ArithmeticFamily::ShiftLeft
                    | ArithmeticFamily::ShiftRight
            ) {
                self.fallible(expression, call)
            } else {
                expression
            };
        }
        let operation = match family {
            ArithmeticFamily::Add => "addition",
            ArithmeticFamily::Subtract => "subtraction",
            ArithmeticFamily::Multiply => "multiplication",
            ArithmeticFamily::Divide => "division",
            ArithmeticFamily::Remainder => "remainder",
            ArithmeticFamily::DivRem => "div_rem",
            ArithmeticFamily::Negate => "negation",
            ArithmeticFamily::ShiftLeft => "shift_left",
            ArithmeticFamily::ShiftRight => "shift_right",
        };
        let helper = if child == "default" {
            format!("fixed_{operation}")
        } else {
            format!("fixed_{operation}_{child}")
        };
        let expression = if family == ArithmeticFamily::Negate {
            format!("terrane_int_support::{helper}({receiver})")
        } else if matches!(
            family,
            ArithmeticFamily::ShiftLeft | ArithmeticFamily::ShiftRight
        ) {
            format!(
                "terrane_int_support::{helper}({receiver}, &({}))",
                argument.unwrap()
            )
        } else {
            format!(
                "terrane_int_support::{helper}({receiver}, {})",
                argument.unwrap()
            )
        };
        let fallible = child == "default"
            || matches!(
                family,
                ArithmeticFamily::Divide
                    | ArithmeticFamily::Remainder
                    | ArithmeticFamily::DivRem
                    | ArithmeticFamily::ShiftLeft
                    | ArithmeticFamily::ShiftRight
            );
        if fallible {
            self.fallible(expression, call)
        } else {
            expression
        }
    }

    fn integer_coercion(
        &mut self,
        method: &crate::BoundMethod,
        receiver: &SyntaxNode,
        callee: &SyntaxNode,
        arguments: &SyntaxNode,
    ) -> String {
        let policy = match method.child {
            "default" => CoercionPolicy::Default,
            child => CoercionPolicy::from_member(child)
                .expect("validated coercion family child must select a policy"),
        };
        let destination = arguments
            .children
            .first()
            .and_then(|argument| argument.children.last())
            .unwrap_or_else(|| &arguments.children[0]);
        let destination = self
            .descriptor_type(destination)
            .expect("validated coercion destination must resolve to a scalar descriptor");
        let receiver_is_borrowed = receiver.kind == SyntaxKind::Name
            && self.lazy_namespace_binding_type(receiver).is_some();
        if policy == CoercionPolicy::Default
            && !receiver_is_borrowed
            && let Some(ValueType::Scalar(source)) = self.value_type(receiver)
        {
            if source == destination {
                return if destination == ScalarType::Int
                    && self.small_int_binding(receiver).is_some()
                {
                    self.adaptive_expression(receiver)
                } else {
                    self.expression(receiver)
                };
            }
            return self.numeric_destination(receiver, source, destination);
        }
        let helper = match policy {
            CoercionPolicy::Default => "coerce",
            CoercionPolicy::Checked => "checked_coerce",
            CoercionPolicy::Wrap => "wrapping_coerce",
            CoercionPolicy::Saturate => "saturating_coerce",
        };
        let receiver = self.expression(receiver);
        let source = if receiver_is_borrowed {
            receiver
        } else {
            format!("&({receiver})")
        };
        let call = format!(
            "terrane_int_support::{helper}::<{}>({source})",
            rust_type(destination)
        );
        if policy == CoercionPolicy::Default {
            self.fallible(call, callee)
        } else {
            call
        }
    }

    fn descriptor_identity(&self, node: &SyntaxNode) -> Option<String> {
        if node.kind == SyntaxKind::TypeExpression {
            return node
                .children
                .first()
                .and_then(|child| self.descriptor_identity(child));
        }
        if node.kind == SyntaxKind::MemberExpression
            && let [receiver, member] = node.children.as_slice()
            && self.text(member) == "type"
        {
            return self
                .value_type(receiver)
                .and_then(|value_type| match value_type {
                    ValueType::Scalar(value_type) => Some(format!("type:{value_type}")),
                    _ => None,
                });
        }
        crate::semantics::descriptor_expression_type(self.package, self.unit, node)
            .map(|scalar| format!("type:{scalar}"))
    }

    fn descriptor_type(&self, node: &SyntaxNode) -> Option<ScalarType> {
        let resolved = crate::semantics::descriptor_expression_type(self.package, self.unit, node);
        resolved.or_else(|| {
            (node.kind == SyntaxKind::TypeExpression)
                .then(|| {
                    node.children
                        .first()
                        .and_then(|child| self.descriptor_type(child))
                })
                .flatten()
        })
    }

    fn contract_for_call(&self, callee: &SyntaxNode) -> Option<&FunctionContract> {
        if callee.kind != SyntaxKind::Name {
            return None;
        }
        let symbol =
            self.package
                .resolve_name_at(self.unit, callee.span.start, self.text(callee))?;
        let span = symbol.declaration_span?;
        self.package
            .units
            .iter()
            .flat_map(|unit| &unit.functions)
            .find(|contract| contract.span == span)
    }

    fn name(&self, node: &SyntaxNode) -> String {
        let source_name = self.text(node);
        if source_name == "none" {
            return "()".to_owned();
        }
        let narrowed =
            narrowed_value_type(self.unit, node, &self.unit.typed_bindings).or_else(|| {
                self.parameter_types
                    .iter()
                    .rev()
                    .find(|(name, _)| name == source_name)
                    .and_then(|(_, value_type)| {
                        narrowed_optional_type(self.unit, node, value_type.clone())
                    })
            });
        if let Some(narrowed) = narrowed {
            let access = format!(
                "{}.as_ref().expect(\"semantic optional narrowing\")",
                rust_name(source_name)
            );
            return if matches!(narrowed, ValueType::Scalar(_)) {
                format!("*{access}")
            } else {
                access
            };
        }
        if let Some((_, local)) = self
            .namespace_initializer
            .as_ref()
            .filter(|(name, _)| name == source_name)
        {
            return local.clone();
        }
        let resolved = self
            .package
            .resolve_name_at(self.unit, node.span.start, source_name);
        let encoding = resolved
            .and_then(|symbol| symbol.identity.strip_prefix("/core/encodings::"))
            .and_then(|name| match name {
                "utf8" => Some("Utf8"),
                "utf16-le" => Some("Utf16Le"),
                "utf16-be" => Some("Utf16Be"),
                "utf32-le" => Some("Utf32Le"),
                "utf32-be" => Some("Utf32Be"),
                _ => None,
            });
        if let Some(encoding) = encoding {
            return format!("terrane_string_support::Encoding::{encoding}");
        }
        let Some(symbol) = self
            .package
            .resolve_name_at(self.unit, node.span.start, source_name)
        else {
            return rust_name(source_name);
        };
        if symbol.kind != SymbolKind::Binding {
            return rust_name(source_name);
        }
        if symbol.global {
            let storage = global_binding_name(&symbol.name);
            let failure = self.uninitialized_global_failure(node);
            return format!(
                "{storage}.lock().expect(\"program-global lock poisoned\").clone().unwrap_or_else(|| {failure})"
            );
        }
        let Some(span) = symbol.declaration_span else {
            return rust_name(source_name);
        };
        let name = namespace_binding_name(span.file, &symbol.name);
        if self.lazy_namespace_binding_type(node).is_some() {
            format!("&*{name}")
        } else if self.is_namespace_binding_span(span) {
            name
        } else {
            rust_name(source_name)
        }
    }

    fn uninitialized_global_failure(&self, node: &SyntaxNode) -> String {
        let (line, column) = self.source.line_column(node.span.start);
        format!(
            "__terrane_uninitialized_global({:?}, {:?}, {line}, {column})",
            self.text(node),
            display_path(self.source.path())
        )
    }

    fn namespace_name(&self, node: &SyntaxNode) -> String {
        self.package
            .resolve_name_at(self.unit, node.span.start, self.text(node))
            .and_then(|symbol| {
                symbol
                    .declaration_span
                    .map(|span| namespace_binding_name(span.file, &symbol.name))
            })
            .unwrap_or_else(|| rust_name(self.text(node)))
    }

    fn small_int_binding(&self, node: &SyntaxNode) -> Option<ScalarType> {
        (node.kind == SyntaxKind::Name)
            .then(|| {
                self.unit
                    .typed_bindings
                    .iter()
                    .rev()
                    .find(|binding| {
                        binding.name == self.text(node)
                            && binding.span.start <= node.span.start
                            && !binding_span_is_mutated(self.package, self.unit, binding.span, true)
                    })
                    .and_then(|binding| binding.storage_type)
            })
            .flatten()
    }

    fn lazy_namespace_binding_type(&self, node: &SyntaxNode) -> Option<ValueType> {
        if self
            .namespace_initializer
            .as_ref()
            .is_some_and(|(name, _)| name == self.text(node))
        {
            return None;
        }
        let symbol = self
            .package
            .resolve_name_at(self.unit, node.span.start, self.text(node))?;
        if symbol.global {
            return None;
        }
        let span = symbol.declaration_span?;
        if !self.is_namespace_binding_span(span) {
            return None;
        }
        let owner = self
            .package
            .units
            .iter()
            .find(|unit| unit.source.id() == span.file)?;
        owner
            .typed_bindings
            .iter()
            .find(|binding| binding.span == span)
            .map(|binding| binding.value_type.clone())
    }

    fn is_namespace_binding_span(&self, span: crate::Span) -> bool {
        self.package
            .units
            .iter()
            .find(|unit| unit.source.id() == span.file)
            .is_some_and(|unit| {
                unit.tree.root.children.iter().any(|candidate| {
                    candidate.span == span
                        && matches!(candidate.kind, SyntaxKind::Binding | SyntaxKind::Assignment)
                })
            })
    }

    fn append_defaults(&self, contract: &FunctionContract, values: &mut [Option<String>]) {
        if values.iter().all(Option::is_some) {
            return;
        }
        let Some(owner) = self
            .package
            .units
            .iter()
            .find(|unit| unit.source.id() == contract.span.file)
        else {
            return;
        };
        let Some(function) = find_node(
            &owner.tree.root,
            SyntaxKind::FunctionDeclaration,
            contract.span,
        ) else {
            return;
        };
        let Some(parameters) = function
            .children
            .iter()
            .find(|child| child.kind == SyntaxKind::ParameterList)
        else {
            return;
        };
        for (index, parameter) in parameters.children.iter().enumerate() {
            if values[index].is_some() {
                continue;
            }
            if let Some(default) = parameter.children.last().filter(|child| {
                !matches!(child.kind, SyntaxKind::Name | SyntaxKind::TypeExpression)
            }) {
                let destination = contract.parameters[index].value_type.clone();
                let value = destination
                    .and_then(|destination| match destination {
                        ValueType::Scalar(destination) => {
                            contextual_constant(&owner.source, default, destination)
                                .and_then(Result::ok)
                                .map(|constant| lower_contextual_constant(constant, destination))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| literal_or_text(&owner.source, default));
                values[index] = Some(value);
            }
        }
    }

    fn is_builtin(&self, node: &SyntaxNode, identity: &str) -> bool {
        let SyntaxKind::Name = node.kind else {
            return false;
        };
        self.package
            .resolve_name_at(self.unit, node.span.start, self.text(node))
            .is_some_and(|symbol| symbol.identity == identity)
    }

    fn text(&self, node: &SyntaxNode) -> &str {
        &self.source.text()[node.span.start..node.span.end]
    }

    fn control_condition(&mut self, mut node: &SyntaxNode) -> String {
        while node.kind == SyntaxKind::GroupExpression
            && let [grouped] = node.children.as_slice()
        {
            node = grouped;
        }
        let expression = self.expression(node);
        if node.kind == SyntaxKind::BinaryExpression {
            Self::unwrapped_expression(expression)
        } else {
            expression
        }
    }

    fn line_start(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }

    fn line(&mut self, text: &str) {
        self.line_start();
        self.output.push_str(text);
        self.output.push('\n');
    }
}

fn find_node(node: &SyntaxNode, kind: SyntaxKind, span: crate::Span) -> Option<&SyntaxNode> {
    if node.kind == kind && node.span == span {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| find_node(child, kind, span))
}

fn literal_or_text(source: &SourceFile, node: &SyntaxNode) -> String {
    let text = &source.text()[node.span.start..node.span.end];
    if node.kind == SyntaxKind::Literal {
        literal(text)
    } else {
        text.trim().to_owned()
    }
}

fn literal(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed == "true" || trimmed == "false" {
        return trimmed.to_owned();
    }
    if trimmed.starts_with("b'") && trimmed.ends_with('\'') {
        let value = crate::lexer::unescape_bytes(&trimmed[2..trimmed.len() - 1])
            .expect("lexer rejects malformed byte escapes before lowering");
        return format!("Vec::from({value:?})");
    }
    let compact = trimmed.replace('_', "");
    if let Some(value) = integer_literal(&compact) {
        return value.to_string();
    }
    if compact.parse::<f64>().is_ok() {
        return compact;
    }
    let value = if let Some(value) = trimmed.strip_prefix('>') {
        if let Some(block) = value.strip_prefix('>') {
            block_string(block)
        } else {
            value.to_owned()
        }
    } else if trimmed.len() >= 2
        && ((trimmed.starts_with('\'') && trimmed.ends_with('\''))
            || (trimmed.starts_with('"') && trimmed.ends_with('"')))
    {
        unescape(&trimmed[1..trimmed.len() - 1])
    } else {
        trimmed.to_owned()
    };
    format!("String::from({value:?})")
}

fn adaptive_literal(text: &str) -> String {
    let compact = text.trim().replace('_', "");
    let value = integer_literal(&compact)
        .expect("semantic analysis accepted a non-integer adaptive literal");
    let decimal = value.to_string();
    if decimal.parse::<i128>().is_ok() {
        format!("terrane_int_support::Int::from({decimal}_i128)")
    } else {
        format!("terrane_int_support::Int::from_decimal({decimal:?})")
    }
}

fn lower_contextual_constant(constant: ContextualConstant, destination: ScalarType) -> String {
    match constant {
        ContextualConstant::Integer(value) if destination == ScalarType::Int => {
            adaptive_literal(&value.to_string())
        }
        ContextualConstant::Integer(value) => value.to_string(),
        ContextualConstant::Float32(value) => float32_literal(value),
        ContextualConstant::Float64(value) => float64_literal(value),
    }
}

fn float32_literal(value: f32) -> String {
    if value.is_nan() {
        "f32::NAN".to_owned()
    } else if value == f32::INFINITY {
        "f32::INFINITY".to_owned()
    } else if value == f32::NEG_INFINITY {
        "f32::NEG_INFINITY".to_owned()
    } else {
        format!("{value:?}_f32")
    }
}

fn float64_literal(value: f64) -> String {
    if value.is_nan() {
        "f64::NAN".to_owned()
    } else if value == f64::INFINITY {
        "f64::INFINITY".to_owned()
    } else if value == f64::NEG_INFINITY {
        "f64::NEG_INFINITY".to_owned()
    } else {
        format!("{value:?}_f64")
    }
}

fn integer_literal(text: &str) -> Option<BigInt> {
    let (radix, digits) =
        if let Some(digits) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            (16, digits)
        } else if let Some(digits) = text.strip_prefix("0o").or_else(|| text.strip_prefix("0O")) {
            (8, digits)
        } else if let Some(digits) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
            (2, digits)
        } else {
            (10, text)
        };
    BigInt::parse_bytes(digits.as_bytes(), radix)
}

fn block_string(text: &str) -> String {
    let mut lines = text.lines();
    let first = lines.next().unwrap_or_default();
    if !first.trim().is_empty() {
        return first.to_owned();
    }
    let collected = lines.collect::<Vec<_>>();
    let indent = collected
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);
    collected
        .iter()
        .map(|line| line.get(indent..).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

fn unescape(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        if character == '\\' {
            match chars.next() {
                Some('n') => output.push('\n'),
                Some('r') => output.push('\r'),
                Some('t') => output.push('\t'),
                Some('\\') | None => output.push('\\'),
                Some('\'') => output.push('\''),
                Some('"') => output.push('"'),
                Some(other) => output.push(other),
            }
        } else {
            output.push(character);
        }
    }
    output
}

fn union_type_name(binding: &TypedBinding) -> String {
    format!("TerraneUnionF{}S{}", binding.span.file, binding.span.start)
}
fn find_node_by_span(node: &SyntaxNode, span: crate::Span) -> Option<&SyntaxNode> {
    (node.span == span).then_some(node).or_else(|| {
        node.children
            .iter()
            .find_map(|child| find_node_by_span(child, span))
    })
}

fn binding_initializer(node: &SyntaxNode, name_index: usize) -> Option<&SyntaxNode> {
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

fn rust_type(ty: ScalarType) -> &'static str {
    ty.lowering_type()
}
#[expect(
    clippy::needless_pass_by_value,
    reason = "element lowering owns the recursively described value type"
)]
fn rust_element_type(ty: ElementType) -> String {
    rust_value_type(ty.value_type())
}

fn rust_value_type(ty: ValueType) -> String {
    match ty {
        ValueType::Scalar(scalar) => rust_type(scalar).to_owned(),
        ValueType::ScalarOrNone(scalar) => format!("Option<{}>", rust_type(scalar)),
        ValueType::OverflowResult(scalar) => {
            format!("terrane_int_support::OverflowResult<{}>", rust_type(scalar))
        }
        ValueType::DivRemResult(scalar) => {
            format!("terrane_int_support::DivRemResult<{}>", rust_type(scalar))
        }
        ValueType::StringView(crate::semantics::TextUnit::Bytes) => "Vec<u8>".to_owned(),
        ValueType::StringView(_) | ValueType::TextRangeView(_) => "String".to_owned(),
        ValueType::StringList => "Vec<String>".to_owned(),
        ValueType::Encoding => "terrane_string_support::Encoding".to_owned(),
        ValueType::TextRange => "terrane_string_support::TextRange".to_owned(),
        ValueType::TextRangeOrNone => "Option<terrane_string_support::TextRange>".to_owned(),
        ValueType::Iterator(item) => {
            format!(
                "terrane_collection_support::Iterator<{}>",
                rust_element_type(item)
            )
        }
        ValueType::IterationStep(item) => {
            format!(
                "terrane_collection_support::IterationStep<{}>",
                rust_element_type(item)
            )
        }
        ValueType::ElementOrNone(item) => {
            format!("Option<{}>", rust_element_type(item))
        }
        ValueType::List(item) => {
            format!(
                "terrane_collection_support::List<{}>",
                rust_element_type(item)
            )
        }
        ValueType::Map(key, value) => format!(
            "terrane_collection_support::Map<{}, {}>",
            rust_element_type(key),
            rust_element_type(value)
        ),
        ValueType::Set(item) => {
            format!(
                "terrane_collection_support::Set<{}>",
                rust_element_type(item)
            )
        }
        ValueType::Tuple(item, _) => {
            format!(
                "terrane_collection_support::Tuple<{}>",
                rust_element_type(item)
            )
        }
        ValueType::Range => "terrane_collection_support::Range".to_owned(),
        ValueType::Entry(key, value) => format!(
            "terrane_collection_support::Entry<{}, {}>",
            rust_element_type(key),
            rust_element_type(value)
        ),
        ValueType::UnorderedMap(key, value) => format!(
            "terrane_collection_support::UnorderedMap<{}, {}>",
            rust_element_type(key),
            rust_element_type(value)
        ),
        ValueType::UnorderedSet(item) => {
            format!(
                "terrane_collection_support::UnorderedSet<{}>",
                rust_element_type(item)
            )
        }
        ValueType::TextRangeList => "Vec<terrane_string_support::TextRange>".to_owned(),
    }
}

const fn is_numeric(ty: ScalarType) -> bool {
    ty.is_integer() || matches!(ty, ScalarType::Float32 | ScalarType::Float64)
}

fn integer_range_contains(destination: ScalarType, source: ScalarType) -> bool {
    let Some((destination_signed, destination_bits)) = fixed_integer_shape(destination) else {
        return false;
    };
    let Some((source_signed, source_bits)) = fixed_integer_shape(source) else {
        return false;
    };
    match (destination_signed, source_signed) {
        (true, true) | (false, false) => destination_bits >= source_bits,
        (true, false) => destination_bits > source_bits,
        (false, true) => false,
    }
}

fn exact_integer_float_widening(source: ScalarType, destination: ScalarType) -> bool {
    let Some((_, bits)) = fixed_integer_shape(source) else {
        return false;
    };
    match destination {
        ScalarType::Float32 => bits <= 16,
        ScalarType::Float64 => bits <= 32,
        _ => false,
    }
}

const fn fixed_integer_shape(ty: ScalarType) -> Option<(bool, u16)> {
    match ty {
        ScalarType::Int8 => Some((true, 8)),
        ScalarType::Int16 => Some((true, 16)),
        ScalarType::Int32 => Some((true, 32)),
        ScalarType::Int64 => Some((true, 64)),
        ScalarType::Int128 => Some((true, 128)),
        ScalarType::Uint8 => Some((false, 8)),
        ScalarType::Uint16 => Some((false, 16)),
        ScalarType::Uint32 => Some((false, 32)),
        ScalarType::Uint64 => Some((false, 64)),
        ScalarType::Uint128 => Some((false, 128)),
        _ => None,
    }
}

fn node_contains_name(source: &SourceFile, node: &SyntaxNode, name: &str) -> bool {
    if node.kind == SyntaxKind::Name && &source.text()[node.span.start..node.span.end] == name {
        return true;
    }
    let children = match node.kind {
        SyntaxKind::Binding => node.children.get(1..).unwrap_or_default(),
        SyntaxKind::MemberExpression => node.children.get(..1).unwrap_or_default(),
        _ => &node.children,
    };
    children
        .iter()
        .any(|child| node_contains_name(source, child, name))
}

fn block_may_fall_through(block: &SyntaxNode) -> bool {
    block.children.last().is_none_or(statement_may_fall_through)
}

fn statement_may_fall_through(statement: &SyntaxNode) -> bool {
    match statement.kind {
        SyntaxKind::ReturnStatement
        | SyntaxKind::ThrowStatement
        | SyntaxKind::BreakStatement
        | SyntaxKind::ContinueStatement => false,
        SyntaxKind::IfStatement => {
            let mut branches = statement.children.iter().skip(1);
            let Some(first) = branches.next() else {
                return true;
            };
            let first_falls_through = first
                .children
                .last()
                .filter(|child| child.kind == SyntaxKind::Block)
                .map_or_else(|| block_may_fall_through(first), block_may_fall_through);
            let mut has_else = false;
            let mut any_falls_through = first_falls_through;
            for branch in branches {
                has_else |= branch.kind == SyntaxKind::ElseClause;
                any_falls_through |= branch
                    .children
                    .last()
                    .filter(|child| child.kind == SyntaxKind::Block)
                    .is_none_or(block_may_fall_through);
            }
            !has_else || any_falls_through
        }
        SyntaxKind::TryStatement => {
            let try_falls_through = statement
                .children
                .first()
                .is_none_or(block_may_fall_through);
            let catch_falls_through = statement
                .children
                .iter()
                .filter(|child| child.kind == SyntaxKind::CatchClause)
                .filter_map(|clause| {
                    clause
                        .children
                        .iter()
                        .find(|child| child.kind == SyntaxKind::Block)
                })
                .any(block_may_fall_through);
            let finally_returns = statement
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::FinallyClause)
                .and_then(|clause| clause.children.first())
                .is_some_and(|block| !block_may_fall_through(block));
            !finally_returns && (try_falls_through || catch_falls_through)
        }
        _ => true,
    }
}

fn rust_error_kind(kind: &str) -> &'static str {
    match kind {
        "arithmetic-overflow" => "ArithmeticOverflow",
        "division-by-zero" => "DivisionByZero",
        "integer-conversion-overflow" => "IntegerConversionOverflow",
        "negative-shift-count" => "NegativeShiftCount",
        "coercion-error" => "CoercionError",
        "decode-error" => "DecodeError",
        "index-error" => "IndexError",
        "missing-key" => "MissingKey",
        "resource-error" => "ResourceError",
        _ => "SourceError",
    }
}

fn error_message(kind: &str) -> &'static str {
    match kind {
        "arithmetic-overflow" => "fixed-width integer arithmetic overflow",
        "division-by-zero" => "integer division by zero",
        "integer-conversion-overflow" => "integer conversion overflow",
        "negative-shift-count" => "negative integer shift count",
        "coercion-error" => "coercion has no compatible result",
        "decode-error" => "invalid byte sequence for selected encoding",
        "index-error" => "collection index is out of range",
        "missing-key" => "collection key is absent",
        "resource-error" => "integer shift count cannot be represented on this target",
        _ => "source error",
    }
}

fn function_name(contract: &FunctionContract) -> String {
    if contract.name == "main" {
        "main".to_owned()
    } else {
        rust_name(&contract.name)
    }
}

fn namespace_binding_name(file: u32, name: &str) -> String {
    format!("__TERRANE_F{file}_{}", rust_name(name).to_uppercase())
}

fn global_binding_name(name: &str) -> String {
    format!("__TERRANE_GLOBAL_{}", rust_name(name).to_uppercase())
}

fn rust_name(name: &str) -> String {
    let readable_identifier = name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    let keyword = matches!(
        name,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
    );
    if readable_identifier && !keyword {
        return name.replace('-', "_");
    }
    let mut output = String::from("__trn_");
    for byte in name.bytes() {
        write!(output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn display_path(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("<memory>")
        .to_owned()
}
