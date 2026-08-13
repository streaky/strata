use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use terrane_compiler::highlight::{Highlight, HighlightKind, highlight};
use terrane_compiler::{Diagnostic as TerraneDiagnostic, SourceFile, Span};
use tokio::sync::RwLock;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::{
    Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, InitializeParams, InitializeResult, InitializedParams, MessageType,
    NumberOrString, Position, Range, SemanticToken, SemanticTokenModifier, SemanticTokenType,
    SemanticTokens, SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions,
    SemanticTokensParams, SemanticTokensResult, SemanticTokensServerCapabilities, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};
use tower_lsp_server::{Client, LanguageServer};

const TOKEN_TYPES: [SemanticTokenType; 11] = [
    SemanticTokenType::COMMENT,
    SemanticTokenType::KEYWORD,
    SemanticTokenType::NUMBER,
    SemanticTokenType::STRING,
    SemanticTokenType::OPERATOR,
    SemanticTokenType::NAMESPACE,
    SemanticTokenType::TYPE,
    SemanticTokenType::FUNCTION,
    SemanticTokenType::PARAMETER,
    SemanticTokenType::PROPERTY,
    SemanticTokenType::VARIABLE,
];

#[derive(Clone, Debug)]
struct Document {
    text: String,
}

#[derive(Debug)]
pub struct Backend {
    client: Client,
    documents: Arc<RwLock<HashMap<Uri, Document>>>,
}

impl Backend {
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn analyze(&self, uri: &Uri, text: &str) {
        let path = uri
            .to_file_path()
            .map_or_else(|| PathBuf::from(uri.as_str()), std::borrow::Cow::into_owned);
        let source = SourceFile::new(0, path, text.to_owned());
        let output = highlight(&source);
        let diagnostics = output
            .diagnostics
            .iter()
            .map(|diagnostic| lsp_diagnostic(&source, diagnostic))
            .collect();
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: TOKEN_TYPES.to_vec(),
                                token_modifiers: vec![SemanticTokenModifier::DECLARATION],
                            },
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            ..Default::default()
                        },
                    ),
                ),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "Terrane Language Server".to_owned(),
                version: Some(terrane_compiler::VERSION.to_owned()),
            }),
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Terrane language server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.documents
            .write()
            .await
            .insert(uri.clone(), Document { text: text.clone() });
        self.analyze(&uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let Some(change) = params.content_changes.into_iter().last() else {
            return;
        };
        let uri = params.text_document.uri;
        let text = change.text;
        self.documents
            .write()
            .await
            .insert(uri.clone(), Document { text: text.clone() });
        self.analyze(&uri, &text).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.write().await.remove(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let documents = self.documents.read().await;
        let Some(document) = documents.get(&uri) else {
            return Ok(None);
        };
        let source = SourceFile::new(0, PathBuf::from(uri.as_str()), document.text.clone());
        let highlights = highlight(&source).highlights;
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: encode_semantic_tokens(source.text(), &highlights),
        })))
    }
}

#[must_use]
pub fn encode_semantic_tokens(text: &str, highlights: &[Highlight]) -> Vec<SemanticToken> {
    let mut encoded = Vec::with_capacity(highlights.len());
    let mut previous_line = 0_u32;
    let mut previous_start = 0_u32;
    for highlight in highlights {
        for span in split_lines(text, highlight.span) {
            let (line, start, length) = utf16_range(text, span);
            let delta_line = line - previous_line;
            let delta_start = if delta_line == 0 {
                start - previous_start
            } else {
                start
            };
            encoded.push(SemanticToken {
                delta_line,
                delta_start,
                length,
                token_type: token_type(highlight.kind),
                token_modifiers_bitset: u32::from(highlight.declaration),
            });
            previous_line = line;
            previous_start = start;
        }
    }
    encoded
}

fn split_lines(text: &str, span: Span) -> impl Iterator<Item = Span> + '_ {
    let file = span.file;
    text[span.start..span.end]
        .split_inclusive('\n')
        .scan(span.start, move |start, part| {
            let content_end = *start + part.trim_end_matches(['\n', '\r']).len();
            let result = (*start < content_end).then_some(Span::new(file, *start, content_end));
            *start += part.len();
            Some(result)
        })
        .flatten()
}

fn utf16_range(text: &str, span: Span) -> (u32, u32, u32) {
    let line = text[..span.start].bytes().filter(|byte| *byte == b'\n').count();
    let line_start = text[..span.start].rfind('\n').map_or(0, |index| index + 1);
    let start = text[line_start..span.start].encode_utf16().count();
    let length = text[span.start..span.end].encode_utf16().count();
    (
        u32::try_from(line).expect("document line fits in LSP position"),
        u32::try_from(start).expect("document column fits in LSP position"),
        u32::try_from(length).expect("token length fits in LSP position"),
    )
}

fn lsp_diagnostic(source: &SourceFile, diagnostic: &TerraneDiagnostic) -> Diagnostic {
    let range = diagnostic
        .primary
        .map_or_else(|| Range::new(Position::new(0, 0), Position::new(0, 0)), |span| {
            let (line, start, length) = utf16_range(source.text(), span);
            Range::new(
                Position::new(line, start),
                Position::new(line, start + length),
            )
        });
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(diagnostic.code.to_owned())),
        source: Some("terrane".to_owned()),
        message: diagnostic.message.clone(),
        ..Default::default()
    }
}

const fn token_type(kind: HighlightKind) -> u32 {
    match kind {
        HighlightKind::Comment => 0,
        HighlightKind::Keyword => 1,
        HighlightKind::Number => 2,
        HighlightKind::String => 3,
        HighlightKind::Operator => 4,
        HighlightKind::Namespace => 5,
        HighlightKind::Type => 6,
        HighlightKind::Function => 7,
        HighlightKind::Parameter => 8,
        HighlightKind::Property => 9,
        HighlightKind::Variable => 10,
    }
}
