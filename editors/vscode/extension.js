// The UNC assembly extension: the TextMate grammar does the colours, and this starts the
// language server, unc-lsp, for everything else. A plain *.s is UNC when its first line is a
// comment naming the UNC; for other *.s files, pick "UNC assembly" in the language picker or
// add `"files.associations": { "*.s": "unc" }` to the workspace settings.
const vscode = require('vscode');
const { LanguageClient } = require('vscode-languageclient/node');

let client;

function settings() {
  const c = vscode.workspace.getConfiguration('unc');
  return { serverPath: c.get('serverPath', 'unc-lsp'), cumulativeCycles: c.get('cumulativeCycles', false) };
}

function activate(context) {
  const s = settings();
  const server = { command: s.serverPath, args: [] };
  client = new LanguageClient('unc-lsp', 'UNC assembly', { run: server, debug: server }, {
    documentSelector: [{ scheme: 'file', language: 'unc' }, { scheme: 'untitled', language: 'unc' }],
    initializationOptions: { cumulativeCycles: s.cumulativeCycles },
    synchronize: { configurationSection: 'unc' },
  });
  client.start();
  context.subscriptions.push(client);
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
