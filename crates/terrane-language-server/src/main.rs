use tokio::io::{stdin, stdout};
use tower_lsp_server::{LspService, Server};

#[tokio::main]
async fn main() {
    let (service, socket) = LspService::new(terrane_language_server::Backend::new);
    Server::new(stdin(), stdout(), socket).serve(service).await;
}
