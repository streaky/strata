use std::fmt::Write as _;

use crate::Span;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Program {
    pub version: &'static str,
    pub globals: Vec<Item>,
    pub modules: Vec<Module>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedFile {
    pub path: String,
    pub contents: String,
    pub associations: Vec<SourceAssociation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceAssociation {
    pub generated_start: usize,
    pub generated_end: usize,
    pub source: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Module {
    pub source_path: String,
    pub namespace: String,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    pub source: Option<Span>,
    pub body: Block,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub expressions: Vec<Expression>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expression {
    Rust(String),
}

impl Item {
    #[must_use]
    pub fn generated(rust: String) -> Self {
        Self {
            source: None,
            body: Block {
                expressions: vec![Expression::Rust(rust)],
            },
        }
    }

    #[must_use]
    pub fn sourced(source: Span, rust: String) -> Self {
        Self {
            source: Some(source),
            body: Block {
                expressions: vec![Expression::Rust(rust)],
            },
        }
    }

    fn render(&self, output: &mut String) {
        for expression in &self.body.expressions {
            match expression {
                Expression::Rust(rust) => output.push_str(rust),
            }
        }
    }

    fn render_associated(&self, output: &mut String, associations: &mut Vec<SourceAssociation>) {
        let generated_start = output.len();
        self.render(output);
        if let Some(source) = self.source {
            associations.push(SourceAssociation {
                generated_start,
                generated_end: output.len(),
                source,
            });
        }
    }
}

impl Program {
    #[must_use]
    pub fn render(&self) -> String {
        let mut output = format!(
            "// Generated deterministically by Terrane {}.\n",
            self.version
        );
        for item in &self.globals {
            item.render(&mut output);
        }
        for module in &self.modules {
            if module.items.is_empty() {
                continue;
            }
            writeln!(
                output,
                "// Source: {}\n// Namespace: {}",
                module.source_path,
                module.namespace.trim_start_matches('/')
            )
            .expect("writing to a String cannot fail");
            for item in &module.items {
                item.render(&mut output);
            }
        }
        output
    }

    #[must_use]
    pub fn render_files(&self) -> Vec<RenderedFile> {
        let mut files = Vec::new();
        let mut entrypoint = format!(
            "// Generated deterministically by Terrane {}.\n",
            self.version
        );
        if !self.globals.is_empty() {
            let mut contents = String::new();
            let mut associations = Vec::new();
            for item in &self.globals {
                item.render_associated(&mut contents, &mut associations);
            }
            files.push(RenderedFile {
                path: "src/authored/globals.rs".to_owned(),
                contents,
                associations,
            });
            entrypoint.push_str("include!(\"authored/globals.rs\");\n");
        }
        for (index, module) in self.modules.iter().enumerate() {
            if module.items.is_empty() {
                continue;
            }
            let mut contents = format!(
                "// Source: {}\n// Namespace: {}\n",
                module.source_path,
                module.namespace.trim_start_matches('/')
            );
            let mut associations = Vec::new();
            for item in &module.items {
                item.render_associated(&mut contents, &mut associations);
            }
            let path = format!("src/authored/unit-{index:04}.rs");
            writeln!(entrypoint, "include!(\"authored/unit-{index:04}.rs\");")
                .expect("writing to a String cannot fail");
            files.push(RenderedFile {
                path,
                contents,
                associations,
            });
        }
        files.push(RenderedFile {
            path: "src/main.rs".to_owned(),
            contents: entrypoint,
            associations: Vec::new(),
        });
        files
    }
}
