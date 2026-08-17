use std::fmt::Write as _;

use num_bigint::BigInt;

use crate::{
    ScalarType, SourceFile,
    semantics::{
        CoercionPolicy, FunctionContract, SemanticPackage, SemanticUnit, SymbolKind, ValueType,
        integer_coercion_call,
    },
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
            return_type: None,
            parameter_types: Vec::new(),
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
}

impl Emitter<'_> {
    fn namespace_binding(&mut self, node: &SyntaxNode) {
        if Self::is_compiler_object_binding(node) {
            return;
        }
        let Some((name_index, name)) = node
            .children
            .iter()
            .enumerate()
            .find(|(_, child)| child.kind == SyntaxKind::Name)
        else {
            return;
        };
        let Some(binding) = self
            .unit
            .typed_bindings
            .iter()
            .find(|binding| binding.span == node.span)
            .or_else(|| {
                (node.kind == SyntaxKind::Assignment).then(|| {
                    self.unit.typed_bindings.iter().rev().find(|binding| {
                        binding.name == self.text(name) && binding.span.start <= node.span.start
                    })
                })?
            })
        else {
            return;
        };
        if matches!(binding.value_type, ValueType::TypeDescriptor(_)) {
            return;
        }
        let Some(initializer) = binding_initializer(node, name_index) else {
            assert!(
                !self.text(node).contains('='),
                "analyzed initialized value binding must have a selected initializer"
            );
            return;
        };
        let ValueType::Scalar(scalar) = binding.value_type else {
            return;
        };
        let ty = rust_type(scalar);
        let value = self.expression_as(initializer, binding.value_type);
        let name = namespace_binding_name(self.source.id(), self.text(name));
        if matches!(scalar, ScalarType::Int | ScalarType::String) {
            self.line(&format!(
                "static {name}: std::sync::LazyLock<{ty}> = std::sync::LazyLock::new(|| {value});"
            ));
        } else {
            self.line(&format!("static {name}: {ty} = {value};"));
        }
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
            write!(self.output, "{}: {ty}", rust_name(&parameter.name)).unwrap();
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
                    let value_type = self.value_type(left);
                    let mut value = if let Some(value_type) = value_type {
                        self.expression_as(right, value_type)
                    } else {
                        self.expression(right)
                    };
                    if value_type == Some(ValueType::Scalar(ScalarType::Int))
                        && right.kind == SyntaxKind::BinaryExpression
                    {
                        value = value
                            .strip_prefix('(')
                            .and_then(|value| value.strip_suffix(')'))
                            .unwrap_or(&value)
                            .to_owned();
                    }
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
        if Self::is_compiler_object_binding(node) {
            return;
        }
        let binding = self
            .unit
            .typed_bindings
            .iter()
            .find(|binding| binding.span == node.span);
        if binding.is_some_and(|binding| matches!(binding.value_type, ValueType::TypeDescriptor(_)))
        {
            return;
        }
        let ty = binding.map(|binding| match binding.value_type {
            ValueType::Scalar(scalar) => rust_type(scalar).to_owned(),
            ValueType::ScalarOrNone(scalar) => format!("Option<{}>", rust_type(scalar)),
            ValueType::TypeDescriptor(_) => "()".to_owned(),
        });
        let initializer = binding_initializer(node, name_index);
        assert!(
            initializer.is_some() || !self.text(node).contains('='),
            "analyzed initialized value binding must have a selected initializer"
        );
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
        match value_type {
            ValueType::Scalar(ScalarType::Int) => self.adaptive_expression(node),
            ValueType::Scalar(ScalarType::Float32)
                if self.value_type(node) == Some(ValueType::Scalar(ScalarType::Float)) =>
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

    fn is_adaptive_expression(&self, node: &SyntaxNode) -> bool {
        self.value_type(node) == Some(ValueType::Scalar(ScalarType::Int))
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
        if self.is_adaptive_expression(left) {
            return self.adaptive_binary(node);
        }
        if matches!(
            self.value_type(left),
            Some(ValueType::Scalar(value_type))
                if value_type.is_integer() && value_type != ScalarType::Int
        ) && let Some(operation) = match source_operator {
            "+" => Some("addition"),
            "-" => Some("subtraction"),
            "*" => Some("multiplication"),
            "/" => Some("division"),
            "%" => Some("remainder"),
            "<<" => Some("shift_left"),
            ">>" => Some("shift_right"),
            _ => None,
        } {
            let right = self.expression(right);
            let right = if matches!(source_operator, "<<" | ">>") {
                format!("&{right}")
            } else {
                right
            };
            return format!(
                "terrane_int_support::unwrap_or_fail(terrane_int_support::fixed_{operation}({}, {right}))",
                self.expression(left),
            );
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
        let value_type = self.value_type(value);
        let descriptor_type = self.descriptor_type(descriptor);
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
        let expression = match value_type {
            Some(value_type) => self.expression_as(value, value_type),
            None => self.expression(value),
        };
        let effect = Self::discarded_expression(expression);
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

    fn discarded_expression(mut expression: String) -> String {
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
        format!("let _ = {expression};")
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
                text if text.contains('.') => Some(ValueType::Scalar(ScalarType::Float)),
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

    fn integer_coercion(&mut self, callee: &SyntaxNode, arguments: &SyntaxNode) -> Option<String> {
        let (receiver, policy) = integer_coercion_call(self.source, callee)?;
        let destination = arguments
            .children
            .first()
            .and_then(|argument| argument.children.last())
            .unwrap_or_else(|| &arguments.children[0]);
        let destination = self.descriptor_type(destination)?;
        let helper = match policy {
            CoercionPolicy::Default => "coerce",
            CoercionPolicy::Checked => "checked_coerce",
            CoercionPolicy::Wrap => "wrapping_coerce",
            CoercionPolicy::Saturate => "saturating_coerce",
        };
        let receiver_is_borrowed = receiver.kind == SyntaxKind::Name
            && self.lazy_namespace_binding_type(receiver).is_some();
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
                    ValueType::ScalarOrNone(_) | ValueType::TypeDescriptor(_) => None,
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
        let name = namespace_binding_name(span.file, &symbol.name);
        if self.lazy_namespace_binding_type(node).is_some() {
            format!("&*{name}")
        } else if self.is_namespace_binding_span(span) {
            name
        } else {
            rust_name(source_name)
        }
    }

    fn namespace_name(&self, node: &SyntaxNode) -> String {
        self.package
            .resolve_ordinary_at(self.unit, node.span.start, self.text(node))
            .and_then(|symbol| {
                symbol
                    .declaration_span
                    .map(|span| namespace_binding_name(span.file, &symbol.name))
            })
            .unwrap_or_else(|| rust_name(self.text(node)))
    }

    fn lazy_namespace_binding_type(&self, node: &SyntaxNode) -> Option<ValueType> {
        let symbol =
            self.package
                .resolve_ordinary_at(self.unit, node.span.start, self.text(node))?;
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
            .filter(|value_type| {
                matches!(
                    value_type,
                    ValueType::Scalar(ScalarType::Int | ScalarType::String)
                )
            })
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
                        | SyntaxKind::DeclarationModifier
                        | SyntaxKind::Visibility
                        | SyntaxKind::DeclarationQualifier
                )
        })
        .map(|(_, child)| child)
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
