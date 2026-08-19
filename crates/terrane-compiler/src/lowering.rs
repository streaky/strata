use std::fmt::Write as _;

use num_bigint::BigInt;

use crate::{
    ScalarType, SourceFile,
    semantics::{
        CoercionPolicy, ContextualConstant, FunctionContract, SemanticPackage, SemanticUnit,
        SymbolKind, TypedBinding, ValueType, binding_span_is_mutated, contextual_constant,
        integer_coercion_call, promoted_integer_type,
    },
    syntax::{SyntaxKind, SyntaxNode},
};

pub(crate) fn emit(package: &SemanticPackage) -> String {
    let mut output = format!(
        "// Generated deterministically by Terrane {}.\n",
        crate::VERSION
    );
    emit_global_storage(package, &mut output);
    for unit in &package.units {
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
        };
        emitter.emit_union_types();
        for node in &unit.tree.root.children {
            match node.kind {
                SyntaxKind::Binding | SyntaxKind::Assignment => emitter.namespace_binding(node),
                SyntaxKind::FunctionDeclaration => emitter.function(node),
                _ => {}
            }
        }
        if !emitter.output.is_empty() {
            writeln!(
                output,
                "// Source: {}\n// Namespace: {}",
                display_path(unit.source.path()),
                unit.namespace.trim_start_matches('/')
            )
            .unwrap();
            output.push_str(&emitter.output);
        }
    }
    output
}

struct Emitter<'a> {
    package: &'a SemanticPackage,
    unit: &'a SemanticUnit,
    source: &'a SourceFile,
    output: String,
    indent: usize,
    continue_label: Option<String>,
    loop_counter: usize,
    return_type: Option<ScalarType>,
    parameter_types: Vec<(String, ScalarType)>,
    namespace_initializer: Option<(String, String)>,
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
        };
        let value_type = unit
            .typed_bindings
            .iter()
            .find(|binding| binding.span == span)
            .map(|binding| binding.value_type)
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
        let ValueType::Scalar(scalar) = binding.value_type else {
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
            .map(|initializer| self.expression_as(initializer, binding.value_type))
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
            let ty = parameter.value_type.map_or("i128", rust_type);
            let mutable = if parameter.mutable { "mut " } else { "" };
            write!(self.output, "{mutable}{}: {ty}", rust_name(&parameter.name)).unwrap();
        }
        self.output.push(')');
        if let Some(return_type) = contract.return_type
            && return_type != ScalarType::None
        {
            write!(self.output, " -> {}", rust_type(return_type)).unwrap();
        }
        self.output.push_str(" {\n");
        let outer_return_type = std::mem::replace(&mut self.return_type, contract.return_type);
        let outer_parameter_types = std::mem::replace(
            &mut self.parameter_types,
            contract
                .parameters
                .iter()
                .filter_map(|parameter| {
                    parameter
                        .value_type
                        .map(|value_type| (parameter.name.clone(), value_type))
                })
                .collect(),
        );
        self.indent += 1;
        if let Some(block) = node
            .children
            .iter()
            .find(|child| child.kind == SyntaxKind::Block)
        {
            self.block(block);
        }
        self.return_type = outer_return_type;
        self.parameter_types = outer_parameter_types;
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
                ValueType::ScalarOrNone(_) => None,
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
            SyntaxKind::Assignment => {
                if self.global_assignment(node) {
                    return;
                }
                if self
                    .unit
                    .typed_bindings
                    .iter()
                    .any(|binding| binding.span == node.span)
                {
                    self.binding(node);
                } else {
                    let [left, right] = node.children.as_slice() else {
                        return;
                    };
                    let union_binding = self.union_binding(left);
                    let value_type = self.value_type(left);
                    let value = if let Some(binding) = union_binding {
                        self.union_value(&binding, right)
                    } else if let Some(value_type) = value_type {
                        self.expression_as(right, value_type)
                    } else {
                        self.expression(right)
                    };
                    let value = Self::unwrapped_expression(value);
                    let target = self.expression(left);
                    self.line(&format!("{target} = {value};"));
                }
            }
            SyntaxKind::CallExpression => {
                let expression = self.expression(node);
                self.line(&format!("{expression};"));
            }
            SyntaxKind::PostfixExpression => self.postfix(node),
            SyntaxKind::IfStatement => self.if_statement(node),
            SyntaxKind::WhileStatement => self.while_statement(node),
            SyntaxKind::ForStatement => self.for_statement(node),
            SyntaxKind::ReturnStatement => {
                if let Some(value) = node.children.first() {
                    let value = if let Some(return_type) = self.return_type {
                        self.expression_as(value, ValueType::Scalar(return_type))
                    } else {
                        self.expression(value)
                    };
                    let value = Self::unwrapped_expression(value);
                    self.line(&format!("return {value};"));
                } else {
                    self.line("return;");
                }
            }
            SyntaxKind::BreakStatement => self.line("break;"),
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
            match binding.value_type {
                ValueType::Scalar(scalar) => rust_type(scalar).to_owned(),
                ValueType::ScalarOrNone(scalar) => format!("Option<{}>", rust_type(scalar)),
            }
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
                self.expression_as(initializer, binding.value_type)
            } else {
                self.expression(initializer)
            };
            let value = Self::unwrapped_expression(value);
            write!(self.output, " = {value}").unwrap();
        }
        self.output.push_str(";\n");
    }

    fn postfix(&mut self, node: &SyntaxNode) {
        let Some(value) = node.children.first() else {
            return;
        };
        let operator = &self.source.text()[value.span.end..node.span.end];
        let operation = if operator.trim() == "++" { "+" } else { "-" };
        if let Some(storage) = self.global_storage(value) {
            let one = if self.is_adaptive_expression(value) {
                "terrane_int_support::Int::from(1_i128)"
            } else {
                "1"
            };
            self.line("{");
            self.indent += 1;
            self.line(&format!(
                "let mut value = {storage}.lock().expect(\"program-global lock poisoned\");"
            ));
            let failure = self.uninitialized_global_failure(value);
            self.line(&format!(
                "*value = Some(value.clone().unwrap_or_else(|| {failure}) {operation} {one});"
            ));
            self.indent -= 1;
            self.line("}");
            return;
        }
        let target = self.expression(value);
        if self.is_adaptive_expression(value) {
            self.line(&format!(
                "{target} = {target}.clone() {operation} terrane_int_support::Int::from(1_i128);"
            ));
        } else {
            self.line(&format!("{target} {operation}= 1;"));
        }
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
        self.block(block);
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
                let mutable = if binding_span_is_mutated(self.package, self.unit, name.span, true) {
                    "mut "
                } else {
                    ""
                };
                let name = rust_name(self.text(name));
                let collection = self.expression(collection);
                self.line(&format!(
                    "for {mutable}{name} in terrane_string_support::graphemes(&{collection}) {{"
                ));
                self.indent += 1;
                let outer_continue = self.continue_label.take();
                self.block(block);
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
                self.block(block);
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
            SyntaxKind::CallExpression => self.call(node),
            SyntaxKind::PostfixExpression => node
                .children
                .first()
                .map_or_else(String::new, |child| self.expression(child)),
            _ => self.text(node).trim().to_owned(),
        }
    }

    fn expression_as(&mut self, node: &SyntaxNode, value_type: ValueType) -> String {
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
            ValueType::ScalarOrNone(_) => None,
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
            let left = Self::unwrapped_expression(self.expression_as(left, operation_type));
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
            return format!(
                "terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_{operation}({left}, {right}))"
            );
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
            if bytes.first() != Some(&b'(') || bytes.last() != Some(&b')') {
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
                    .map(|binding| binding.value_type)
                    .or_else(|| {
                        self.parameter_types
                            .iter()
                            .find(|(parameter, _)| parameter == name)
                            .map(|(_, value_type)| ValueType::Scalar(*value_type))
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

    fn member(&mut self, node: &SyntaxNode) -> String {
        let [receiver, member] = node.children.as_slice() else {
            return String::new();
        };
        let receiver_type = self.value_type(receiver);
        let receiver = self.expression(receiver);
        match self.text(member) {
            "length" => format!("terrane_string_support::length(&{receiver}) as i128"),
            "type" => "()".to_owned(),
            mode @ ("round" | "floor" | "ceiling" | "truncate")
                if matches!(
                    receiver_type,
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

    fn call(&mut self, node: &SyntaxNode) -> String {
        let [callee, arguments] = node.children.as_slice() else {
            return String::new();
        };
        if let Some(coercion) = self.integer_coercion(callee, arguments) {
            return coercion;
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
                ordered[index] = Some(if let Some(ty) = parameter.value_type {
                    self.expression_as(value, ValueType::Scalar(ty))
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
        format!("{name}({})", values.join(", "))
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
                return format!(
                    "terrane_int_support::unwrap_or_fail(terrane_int_support::{helper}({value}))"
                );
            }
            return if source == ScalarType::Uint128 {
                format!("terrane_int_support::adaptive(&({value}))")
            } else {
                format!("terrane_int_support::Int::from(({value}) as i128)")
            };
        }
        if source == ScalarType::Int && destination.is_integer() {
            return format!(
                "terrane_int_support::unwrap_or_fail(terrane_int_support::coerce::<{}>(&({value})))",
                rust_type(destination)
            );
        }
        if source == ScalarType::Int {
            let helper = if destination == ScalarType::Float32 {
                "exact_f32"
            } else {
                "exact_f64"
            };
            return format!(
                "terrane_int_support::unwrap_or_fail(terrane_int_support::{helper}(&({value})))"
            );
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
            return format!(
                "terrane_int_support::unwrap_or_fail(terrane_int_support::{helper}(&({value})))"
            );
        }
        if destination.is_integer() {
            let helper = if source == ScalarType::Float32 {
                "exact_from_f32"
            } else {
                "exact_from_f64"
            };
            return format!(
                "terrane_int_support::unwrap_or_fail(terrane_int_support::{helper}::<{}>({value}))",
                rust_type(destination)
            );
        }
        format!(
            "{{ let source_value = {value}; let converted = source_value as f32; if (converted as f64) == source_value {{ converted }} else {{ terrane_int_support::unwrap_or_fail(Err(terrane_int_support::ArithmeticError::conversion_overflow(&source_value, \"float64\", \"float32\", \"the floating value is not exactly representable\"))) }} }}"
        )
    }

    fn integer_coercion(&mut self, callee: &SyntaxNode, arguments: &SyntaxNode) -> Option<String> {
        let (receiver, policy) = integer_coercion_call(self.source, callee)?;
        let destination = arguments
            .children
            .first()
            .and_then(|argument| argument.children.last())
            .unwrap_or_else(|| &arguments.children[0]);
        let destination = self.descriptor_type(destination)?;
        let receiver_is_borrowed = receiver.kind == SyntaxKind::Name
            && self.lazy_namespace_binding_type(receiver).is_some();
        if policy == CoercionPolicy::Default
            && !receiver_is_borrowed
            && let Some(ValueType::Scalar(source)) = self.value_type(receiver)
        {
            if source == destination {
                return Some(
                    if destination == ScalarType::Int && self.small_int_binding(receiver).is_some()
                    {
                        self.adaptive_expression(receiver)
                    } else {
                        self.expression(receiver)
                    },
                );
            }
            return Some(self.numeric_destination(receiver, source, destination));
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
        Some(if policy == CoercionPolicy::Default {
            format!("terrane_int_support::unwrap_or_fail({call})")
        } else {
            call
        })
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
                    ValueType::ScalarOrNone(_) => None,
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
        if let Some((_, local)) = self
            .namespace_initializer
            .as_ref()
            .filter(|(name, _)| name == source_name)
        {
            return local.clone();
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
            .map(|binding| binding.value_type)
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
                let destination = contract.parameters[index].value_type;
                let value = destination
                    .and_then(|destination| {
                        contextual_constant(&owner.source, default, destination)
                            .and_then(Result::ok)
                            .map(|constant| lower_contextual_constant(constant, destination))
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

fn function_name(contract: &FunctionContract) -> String {
    rust_name(&contract.name)
}

fn namespace_binding_name(file: u32, name: &str) -> String {
    format!("__TERRANE_F{file}_{}", rust_name(name).to_uppercase())
}

fn global_binding_name(name: &str) -> String {
    format!("__TERRANE_GLOBAL_{}", rust_name(name).to_uppercase())
}

fn rust_name(name: &str) -> String {
    let mut output = String::with_capacity(name.len());
    for character in name.chars() {
        if character == '-' {
            output.push('_');
        } else {
            output.push(character);
        }
    }
    output
}

fn display_path(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("<memory>")
        .to_owned()
}
