use std::fmt::Write as _;

use crate::{
    ScalarType, SourceFile,
    semantics::{FunctionContract, SemanticPackage, SemanticUnit, SymbolKind, ValueType},
    syntax::{SyntaxKind, SyntaxNode},
};

pub(crate) fn emit(package: &SemanticPackage) -> String {
    let mut output = format!(
        "// Generated deterministically by Terrane {}.\n",
        crate::VERSION
    );
    for unit in &package.units {
        let mut emitter = Emitter {
            package,
            unit,
            source: &unit.source,
            output: String::new(),
            indent: 0,
            continue_label: None,
            loop_counter: 0,
        };
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
                unit.namespace.trim_start_matches('/').replace('/', " ")
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
}

impl Emitter<'_> {
    fn namespace_binding(&mut self, node: &SyntaxNode) {
        if Self::is_compiler_object_binding(node) {
            return;
        }
        let Some(binding) = self
            .unit
            .typed_bindings
            .iter()
            .find(|binding| binding.span == node.span)
        else {
            return;
        };
        let Some(name) = node
            .children
            .iter()
            .find(|child| child.kind == SyntaxKind::Name)
        else {
            return;
        };
        let Some(initializer) = node.children.iter().rev().find(|child| {
            !matches!(
                child.kind,
                SyntaxKind::Name
                    | SyntaxKind::TypeExpression
                    | SyntaxKind::DeclarationModifier
                    | SyntaxKind::Visibility
                    | SyntaxKind::DeclarationQualifier
            )
        }) else {
            return;
        };
        let ty = match binding.value_type {
            ValueType::Scalar(scalar) => rust_type(scalar),
            ValueType::ScalarOrNone(_) | ValueType::TypeDescriptor(_) => return,
        };
        let value = self.expression_as(initializer, binding.value_type);
        let name = namespace_binding_name(self.source.id(), self.text(name));
        self.line(&format!("static {name}: {ty} = {value};"));
    }
    fn function(&mut self, node: &SyntaxNode) {
        let Some(contract) = self
            .unit
            .functions
            .iter()
            .find(|item| item.span == node.span)
        else {
            return;
        };
        self.line_start();
        let name = function_name(contract);
        write!(self.output, "fn {name}(").unwrap();
        for (index, parameter) in contract.parameters.iter().enumerate() {
            if index != 0 {
                self.output.push_str(", ");
            }
            let ty = parameter.value_type.map_or("i128", rust_type);
            write!(self.output, "{}: {ty}", rust_name(&parameter.name)).unwrap();
        }
        self.output.push(')');
        if let Some(return_type) = contract.return_type
            && return_type != ScalarType::None
        {
            write!(self.output, " -> {}", rust_type(return_type)).unwrap();
        }
        self.output.push_str(" {\n");
        self.indent += 1;
        if let Some(block) = node
            .children
            .iter()
            .find(|child| child.kind == SyntaxKind::Block)
        {
            self.block(block);
        }
        self.indent -= 1;
        self.line("}");
    }

    fn block(&mut self, block: &SyntaxNode) {
        for statement in &block.children {
            self.statement(statement);
        }
    }

    fn statement(&mut self, node: &SyntaxNode) {
        match node.kind {
            SyntaxKind::Binding => self.binding(node),
            SyntaxKind::Assignment => {
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
                    let value = if self.is_adaptive_expression(left) {
                        self.adaptive_expression(right)
                    } else {
                        self.expression(right)
                    };
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
                    let value = self.expression(value);
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
        let Some(name_node) = node
            .children
            .iter()
            .find(|child| child.kind == SyntaxKind::Name)
        else {
            return;
        };
        let name = rust_name(self.text(name_node));
        if Self::is_compiler_object_binding(node) {
            return;
        }
        let binding = self
            .unit
            .typed_bindings
            .iter()
            .find(|binding| binding.span == node.span);
        let ty = binding.map(|binding| match binding.value_type {
            ValueType::Scalar(scalar) => rust_type(scalar).to_owned(),
            ValueType::ScalarOrNone(scalar) => format!("Option<{}>", rust_type(scalar)),
            ValueType::TypeDescriptor(_) => "()".to_owned(),
        });
        let initializer = node.children.iter().rev().find(|child| {
            !matches!(
                child.kind,
                SyntaxKind::Name
                    | SyntaxKind::TypeExpression
                    | SyntaxKind::DeclarationModifier
                    | SyntaxKind::Visibility
                    | SyntaxKind::DeclarationQualifier
            )
        });
        let mutable = binding.is_some_and(|binding| binding.mutable);
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
            let initializer = if let Some(binding) = binding {
                self.expression_as(initializer, binding.value_type)
            } else {
                self.expression(initializer)
            };
            write!(self.output, " = {initializer}").unwrap();
        }
        self.output.push_str(";\n");
    }

    fn is_compiler_object_binding(node: &SyntaxNode) -> bool {
        node.children
            .last()
            .is_some_and(|child| child.kind == SyntaxKind::ObjectName)
    }

    fn postfix(&mut self, node: &SyntaxNode) {
        let Some(value) = node.children.first() else {
            return;
        };
        let operator = &self.source.text()[value.span.end..node.span.end];
        let operation = if operator.trim() == "++" { "+" } else { "-" };
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
                let name = rust_name(self.text(name));
                let collection = self.expression(collection);
                self.line(&format!(
                    "for {name} in terrane_string_support::graphemes(&{collection}) {{"
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
        if value_type == ValueType::Scalar(ScalarType::Int) {
            self.adaptive_expression(node)
        } else {
            self.expression(node)
        }
    }

    fn adaptive_expression(&mut self, node: &SyntaxNode) -> String {
        match node.kind {
            SyntaxKind::Literal => adaptive_literal(self.text(node)),
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

    fn is_adaptive_expression(&self, node: &SyntaxNode) -> bool {
        match node.kind {
            SyntaxKind::Literal => self
                .text(node)
                .chars()
                .all(|character| character.is_ascii_digit() || character == '_'),
            SyntaxKind::Name => {
                let name = self.text(node).trim();
                self.unit.typed_bindings.iter().rev().any(|binding| {
                    binding.name == name
                        && binding.span.start <= node.span.start
                        && binding.value_type == ValueType::Scalar(ScalarType::Int)
                })
            }
            SyntaxKind::GroupExpression | SyntaxKind::UnaryExpression => node
                .children
                .last()
                .is_some_and(|child| self.is_adaptive_expression(child)),
            SyntaxKind::BinaryExpression => node
                .children
                .first()
                .is_some_and(|child| self.is_adaptive_expression(child)),
            _ => false,
        }
    }

    fn binary(&mut self, node: &SyntaxNode) -> String {
        let [left, right] = node.children.as_slice() else {
            return String::new();
        };
        let source_operator = self.source.text()[left.span.end..right.span.start].trim();
        if source_operator == "is" {
            return matches!(
                (self.descriptor_type(left), self.descriptor_type(right)),
                (Some(left), Some(right)) if left == right
            )
            .to_string();
        }
        if self.is_adaptive_expression(left) {
            return self.adaptive_binary(node);
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

    fn type_membership(&self, node: &SyntaxNode) -> String {
        let [value, descriptor] = node.children.as_slice() else {
            return String::new();
        };
        matches!(
            (self.value_type(value), self.descriptor_type(descriptor)),
            (Some(ValueType::Scalar(value)), Some(descriptor)) if value == descriptor
        )
        .to_string()
    }

    fn value_type(&self, node: &SyntaxNode) -> Option<ValueType> {
        match node.kind {
            SyntaxKind::Literal => match self.text(node).trim() {
                "true" | "false" => Some(ValueType::Scalar(ScalarType::Bool)),
                text if text.starts_with('\'') || text.starts_with('>') => {
                    Some(ValueType::Scalar(ScalarType::String))
                }
                text if text
                    .chars()
                    .all(|character| character.is_ascii_digit() || character == '_') =>
                {
                    Some(ValueType::Scalar(ScalarType::Int))
                }
                _ => None,
            },
            SyntaxKind::Name => self
                .unit
                .typed_bindings
                .iter()
                .rev()
                .find(|binding| {
                    binding.name == self.text(node).trim() && binding.span.start <= node.span.start
                })
                .map(|binding| binding.value_type),
            SyntaxKind::GroupExpression | SyntaxKind::UnaryExpression => node
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
        let receiver = self.expression(receiver);
        match self.text(member) {
            "length" => format!("terrane_string_support::length(&{receiver}) as i128"),
            "type" => "()".to_owned(),
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

    fn integer_coercion(&mut self, callee: &SyntaxNode, arguments: &SyntaxNode) -> Option<String> {
        let [receiver, operation] = callee.children.as_slice() else {
            return None;
        };
        if callee.kind != SyntaxKind::MemberExpression {
            return None;
        }
        let operation = self.text(operation).to_owned();
        if !matches!(
            operation.as_str(),
            "coerce" | "checked-coerce" | "wrapping-coerce" | "saturating-coerce"
        ) {
            return None;
        }
        let destination = arguments
            .children
            .first()
            .and_then(|argument| argument.children.last())
            .unwrap_or_else(|| &arguments.children[0]);
        let destination = self.descriptor_type(destination)?;
        let helper = operation.replace('-', "_");
        let receiver = self.expression(receiver);
        let call = format!(
            "terrane_int_support::{helper}::<{}>(&({receiver}))",
            rust_type(destination)
        );
        Some(if operation == "coerce" {
            format!("terrane_int_support::unwrap_or_fail({call})")
        } else {
            call
        })
    }

    fn descriptor_type(&self, node: &SyntaxNode) -> Option<ScalarType> {
        if node.kind == SyntaxKind::TypeExpression {
            return node
                .children
                .first()
                .and_then(|child| self.descriptor_type(child));
        }
        let name = self.text(node).trim().trim_start_matches('.');
        if let Some(ty) = ScalarType::from_source_name(name) {
            return Some(ty);
        }
        self.unit
            .typed_bindings
            .iter()
            .rev()
            .find(|binding| binding.name == name && binding.span.start <= node.span.start)
            .and_then(|binding| match binding.value_type {
                ValueType::TypeDescriptor(ty) => Some(ty),
                _ => None,
            })
            .or_else(|| {
                self.package
                    .resolve_object_at(self.unit, node.span.start, name)
                    .and_then(crate::semantics::Symbol::descriptor_type)
            })
    }

    fn contract_for_call(&self, callee: &SyntaxNode) -> Option<&FunctionContract> {
        if callee.kind != SyntaxKind::Name {
            return None;
        }
        let symbol =
            self.package
                .resolve_ordinary_at(self.unit, callee.span.start, self.text(callee))?;
        let span = symbol.declaration_span?;
        self.package
            .units
            .iter()
            .flat_map(|unit| &unit.functions)
            .find(|contract| contract.span == span)
    }

    fn name(&self, node: &SyntaxNode) -> String {
        let source_name = self.text(node);
        let Some(symbol) =
            self.package
                .resolve_ordinary_at(self.unit, node.span.start, source_name)
        else {
            return rust_name(source_name);
        };
        if symbol.kind != SymbolKind::Binding {
            return rust_name(source_name);
        }
        let Some(span) = symbol.declaration_span else {
            return rust_name(source_name);
        };
        let is_namespace_binding = self
            .package
            .units
            .iter()
            .find(|unit| unit.source.id() == span.file)
            .is_some_and(|unit| {
                unit.tree.root.children.iter().any(|candidate| {
                    candidate.span == span
                        && matches!(candidate.kind, SyntaxKind::Binding | SyntaxKind::Assignment)
                })
            });
        if is_namespace_binding {
            namespace_binding_name(span.file, &symbol.name)
        } else {
            rust_name(source_name)
        }
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
                !matches!(
                    child.kind,
                    SyntaxKind::Name | SyntaxKind::TypeExpression | SyntaxKind::ObjectName
                )
            }) {
                let value = literal_or_text(&owner.source, default);
                values[index] = Some(
                    if contract.parameters[index].value_type == Some(ScalarType::Int) {
                        adaptive_literal(&value)
                    } else {
                        value
                    },
                );
            }
        }
    }

    fn is_builtin(&self, node: &SyntaxNode, identity: &str) -> bool {
        let SyntaxKind::Name = node.kind else {
            return false;
        };
        let Some(symbol) =
            self.package
                .resolve_ordinary_at(self.unit, node.span.start, self.text(node))
        else {
            return false;
        };
        if symbol.identity == identity {
            return true;
        }
        let Some(declaration) = symbol.declaration_span else {
            return false;
        };
        let Some(owner) = self
            .package
            .units
            .iter()
            .find(|unit| unit.source.id() == declaration.file)
        else {
            return false;
        };
        let binding = find_node(&owner.tree.root, SyntaxKind::Binding, declaration)
            .or_else(|| find_node(&owner.tree.root, SyntaxKind::Assignment, declaration));
        let Some(object) = binding.and_then(|binding| {
            binding
                .children
                .iter()
                .find(|child| child.kind == SyntaxKind::ObjectName)
        }) else {
            return false;
        };
        let name = owner.source.text()[object.span.start..object.span.end].trim_start_matches('.');
        self.package
            .resolve_object_at(owner, object.span.start, name)
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
        if node.kind == SyntaxKind::BinaryExpression
            && let Some(inner) = expression
                .strip_prefix('(')
                .and_then(|value| value.strip_suffix(')'))
        {
            return inner.to_owned();
        }
        expression
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
    if trimmed == "true" || trimmed == "false" || trimmed.parse::<i128>().is_ok() {
        return trimmed.to_owned();
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
    let trimmed = text.trim();
    if trimmed.parse::<i128>().is_ok() {
        format!("terrane_int_support::Int::from({trimmed}_i128)")
    } else {
        format!("terrane_int_support::Int::from_decimal({trimmed:?})")
    }
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

fn rust_type(ty: ScalarType) -> &'static str {
    ty.lowering_type()
}

fn function_name(contract: &FunctionContract) -> String {
    rust_name(&contract.name)
}

fn namespace_binding_name(file: u32, name: &str) -> String {
    format!("__TERRANE_F{file}_{}", rust_name(name).to_uppercase())
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
