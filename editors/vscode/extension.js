'use strict';

const fs = require('fs');
const path = require('path');
const vscode = require('vscode');
const { LanguageClient } = require('vscode-languageclient/node');

let client;

function serverPath(context) {
  const configured = vscode.workspace
    .getConfiguration('terrane')
    .get('server.path', '')
    .trim();
  if (configured) {
    return path.resolve(configured);
  }
  const executable = process.platform === 'win32'
    ? 'terrane-language-server.exe'
    : 'terrane-language-server';
  return context.asAbsolutePath(path.join('server', executable));
}

async function activate(context) {
  const command = serverPath(context);
  if (!fs.existsSync(command)) {
    const message = `Terrane language server not found at ${command}. Configure terrane.server.path or package the server binary.`;
    vscode.window.showErrorMessage(message);
    throw new Error(message);
  }

  client = new LanguageClient(
    'terraneLanguageServer',
    'Terrane Language Server',
    { command },
    {
      documentSelector: [{ scheme: 'file', language: 'terrane' }],
      synchronize: {
        configurationSection: 'terrane',
      },
    },
  );
  await client.start();
  context.subscriptions.push(client);
}

async function deactivate() {
  if (client) {
    await client.stop();
    client = undefined;
  }
}

module.exports = { activate, deactivate, serverPath };
