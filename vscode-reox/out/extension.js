"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = require("vscode");
function activate(context) {
    console.log('Reox extension is active');
    const provider = vscode.languages.registerCompletionItemProvider('reox', {
        provideCompletionItems(document, position, token, context) {
            // Basic keywords
            const keywords = [
                'fn', 'let', 'struct', 'kind', 'layer', 'panel', 'action', 'signal', 'emit',
                'if', 'else', 'while', 'for', 'in', 'match', 'when', 'return', 'break', 'continue',
                'async', 'await', 'guard', 'defer', 'try', 'catch', 'throw',
                'mut', 'pub', 'const', 'static', 'ai', 'assistant'
            ];
            // Native types
            const types = [
                'int', 'float', 'bool', 'string', 'void', 'char', 'byte',
                'int8', 'int16', 'int32', 'int64', 'uint8', 'uint16', 'uint32', 'uint64',
                'float32', 'float64'
            ];
            // Built-in functions (Standard Library)
            const builtins = [
                'print', 'println', 'len', 'type', 'assert', 'panic',
                'vstack', 'hstack', 'zstack', 'text', 'button', 'input', 'list',
                'color_rgba', 'color_hex'
            ];
            const completionItems = [
                ...keywords.map(k => new vscode.CompletionItem(k, vscode.CompletionItemKind.Keyword)),
                ...types.map(t => new vscode.CompletionItem(t, vscode.CompletionItemKind.Class)), // or Struct
                ...builtins.map(b => new vscode.CompletionItem(b, vscode.CompletionItemKind.Function))
            ];
            // Context-aware suggestions could go here
            // For example, suggesting 'await' only inside 'async fn' contexts? 
            // (Simple check: if line contains 'await', ignore. If previous word is 'let', suggest identifiers?)
            return completionItems;
        }
    });
    // Hover Provider
    const hoverProvider = vscode.languages.registerHoverProvider('reox', {
        provideHover(document, position, token) {
            const range = document.getWordRangeAtPosition(position);
            const word = document.getText(range);
            if (word === 'async') {
                return new vscode.Hover('**async**: Defines an asynchronous function that can be awaited.');
            }
            if (word === 'await') {
                return new vscode.Hover('**await**: Pauses execution until the awaited future completes.');
            }
            if (word === 'fn') {
                return new vscode.Hover('**fn**: Declares a function.');
            }
            return undefined;
        }
    });
    context.subscriptions.push(provider, hoverProvider);
}
function deactivate() { }
//# sourceMappingURL=extension.js.map