use std::io::{BufRead, BufReader, Read, Write};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

fn send(stdin: &mut ChildStdin, message: &Value) {
    let body = message.to_string();
    write!(stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
    stdin.flush().unwrap();
}

fn receive(stdout: &mut BufReader<ChildStdout>) -> Value {
    let mut content_length = None;
    loop {
        let mut header = String::new();
        stdout.read_line(&mut header).unwrap();
        assert!(!header.is_empty(), "language server closed stdout");
        if header == "\r\n" {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length: ") {
            content_length = Some(value.trim().parse::<usize>().unwrap());
        }
    }
    let mut body = vec![0; content_length.unwrap()];
    stdout.read_exact(&mut body).unwrap();
    serde_json::from_slice(&body).unwrap()
}

fn receive_response(stdout: &mut BufReader<ChildStdout>, id: u64) -> Value {
    loop {
        let message = receive(stdout);
        if message.get("id").and_then(Value::as_u64) == Some(id) {
            return message;
        }
    }
}

#[test]
fn serves_semantic_tokens_for_an_open_document() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_terrane-language-server"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    send(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"capabilities": {}}
        }),
    );
    let initialized = receive_response(&mut stdout, 1);
    assert_eq!(initialized["result"]["capabilities"]["textDocumentSync"], 1);
    assert_eq!(
        initialized["result"]["capabilities"]["semanticTokensProvider"]["full"],
        true
    );

    send(
        &mut stdin,
        &json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
    send(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": "file:///tmp/editor.trn",
                    "languageId": "terrane",
                    "version": 1,
                    "text": "function main\n  value = >hello\n"
                }
            }
        }),
    );
    send(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/semanticTokens/full",
            "params": {"textDocument": {"uri": "file:///tmp/editor.trn"}}
        }),
    );
    let tokens = receive_response(&mut stdout, 2);
    let data = tokens["result"]["data"].as_array().unwrap();
    assert!(!data.is_empty());
    assert_eq!(data.len() % 5, 0);

    send(
        &mut stdin,
        &json!({"jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": null}),
    );
    let _ = receive_response(&mut stdout, 3);
    send(
        &mut stdin,
        &json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
    );
    drop(stdin);
    assert!(child.wait().unwrap().success());
}
