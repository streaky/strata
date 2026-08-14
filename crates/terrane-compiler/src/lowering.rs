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
        let value = self.expression(initializer);
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
                    let value = self.expression(right);
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
            SyntaxKind::ContinueStatement => self.line("continue;"),
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
        let mutable = self.is_mutated(node, name_node);
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
            let initializer = self.expression(initializer);
            write!(self.output, " = {initializer}").unwrap();
        }
        self.output.push_str(";\n");
    }

    fn is_compiler_object_binding(node: &SyntaxNode) -> bool {
        node.children
            .last()
            .is_some_and(|child| child.kind == SyntaxKind::ObjectName)
    }

    fn is_mutated(&self, declaration: &SyntaxNode, name: &SyntaxNode) -> bool {
        let target = self.text(name);
        contains_mutation_after(
            &self.unit.tree.root,
            declaration.span.end,
            target,
            self.source,
        )
    }

    fn postfix(&mut self, node: &SyntaxNode) {
        let Some(value) = node.children.first() else {
            return;
        };
        let operator = &self.source.text()[value.span.end..node.span.end];
        let operation = if operator.trim() == "++" { "+=" } else { "-=" };
        let value = self.expression(value);
        self.line(&format!("{value} {operation} 1;"));
    }

    fn if_statement(&mut self, node: &SyntaxNode) {
        let Some(condition) = node.children.first() else {
            return;
        };
        let Some(block) = node.children.get(1) else {
            return;
        };
        let condition = control_condition(self.expression(condition));
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
                let condition = control_condition(self.expression(condition));
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
        let condition = control_condition(self.expression(condition));
        self.line(&format!("while {condition} {{"));
        self.indent += 1;
        self.block(block);
        self.indent -= 1;
        self.line("}");
    }

    fn for_statement(&mut self, node: &SyntaxNode) {
        if let [initial, condition, update, block] = node.children.as_slice() {
            self.statement(initial);
            let condition = control_condition(self.expression(condition));
            self.line(&format!("while {condition} {{"));
            self.indent += 1;
            self.block(block);
            self.statement(update);
            self.indent -= 1;
            self.line("}");
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
                let operator = self.source.text()[node.span.start..operand.span.start].trim();
                let operator = match operator {
                    "not" => "!",
                    other => other,
                };
                format!("{operator}{}", self.expression(operand))
            }
            SyntaxKind::BinaryExpression | SyntaxKind::TypeMembershipExpression => {
                self.binary(node)
            }
            SyntaxKind::MemberExpression => self.member(node),
            SyntaxKind::CallExpression => self.call(node),
            SyntaxKind::PostfixExpression => node
                .children
                .first()
                .map_or_else(String::new, |child| self.expression(child)),
            _ => self.text(node).trim().to_owned(),
        }
    }

    fn binary(&mut self, node: &SyntaxNode) -> String {
        let [left, right] = node.children.as_slice() else {
            return String::new();
        };
        let source_operator = self.source.text()[left.span.end..right.span.start].trim();
        if source_operator == "is" {
            return "false".to_owned();
        }
        if source_operator == "is a" {
            let descriptor = self.text(right).trim();
            let Some(binding) = self.unit.typed_bindings.iter().rev().find(|item| {
                item.name == self.text(left).trim() && item.span.start <= left.span.start
            }) else {
                return "false".to_owned();
            };
            let matches = matches!(binding.value_type, ValueType::Scalar(ty) if ty.source_name() == descriptor);
            return matches.to_string();
        }
        let operator = match source_operator {
            "and" => "&&",
            "or" => "||",
            "//" => "/",
            other => other,
        };
        format!(
            "({} {operator} {})",
            self.expression(left),
            self.expression(right)
        )
    }

    fn member(&mut self, node: &SyntaxNode) -> String {
        let [receiver, member] = node.children.as_slice() else {
            return String::new();
        };
        let receiver = self.expression(receiver);
        match self.text(member) {
            "length" => format!("{receiver}.chars().count()"),
            "type" => "()".to_owned(),
            name => format!("{receiver}.{}", rust_name(name)),
        }
    }

    fn call(&mut self, node: &SyntaxNode) -> String {
        let [callee, arguments] = node.children.as_slice() else {
            return String::new();
        };
        let mut values = arguments
            .children
            .iter()
            .map(|argument| argument.children.last().unwrap_or(argument))
            .map(|value| self.expression(value))
            .collect::<Vec<_>>();
        if callee.kind == SyntaxKind::Name && self.text(callee) == "print" {
            if values.is_empty() {
                return "println!()".to_owned();
            }
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
            let format = "{}".repeat(values.len());
            return format!("format!(\"{format}\", {})", values.join(", "));
        }
        let contract = self.contract_for_call(callee).cloned();
        let name = contract
            .as_ref()
            .map_or_else(|| self.expression(callee), function_name);
        if let Some(contract) = contract {
            self.append_defaults(&contract, arguments.children.len(), &mut values);
        }
        format!("{name}({})", values.join(", "))
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

    fn append_defaults(
        &self,
        contract: &FunctionContract,
        supplied: usize,
        values: &mut Vec<String>,
    ) {
        if supplied >= contract.parameters.len() {
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
        for parameter in parameters.children.iter().skip(supplied) {
            if let Some(default) = parameter.children.last().filter(|child| {
                !matches!(
                    child.kind,
                    SyntaxKind::Name | SyntaxKind::TypeExpression | SyntaxKind::ObjectName
                )
            }) {
                values.push(literal_or_text(&owner.source, default));
            }
        }
    }

    fn text(&self, node: &SyntaxNode) -> &str {
        &self.source.text()[node.span.start..node.span.end]
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

fn control_condition(mut expression: String) -> String {
    if expression.starts_with('(') && expression.ends_with(')') {
        expression.remove(0);
        expression.pop();
    }
    expression
}

fn contains_mutation_after(
    node: &SyntaxNode,
    offset: usize,
    name: &str,
    source: &SourceFile,
) -> bool {
    if node.span.start >= offset
        && matches!(
            node.kind,
            SyntaxKind::Assignment | SyntaxKind::PostfixExpression
        )
        && node.children.first().is_some_and(|target| {
            target.kind == SyntaxKind::Name
                && &source.text()[target.span.start..target.span.end] == name
        })
    {
        return true;
    }
    node.children
        .iter()
        .any(|child| contains_mutation_after(child, offset, name, source))
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
    ty.rust_type().unwrap_or(match ty {
        ScalarType::String => "String",
        ScalarType::None => "()",
        _ => "i128",
    })
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
