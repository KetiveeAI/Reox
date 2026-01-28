"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = require("vscode");
function activate(context) {
    console.log('Reox extension is active');
    const provider = vscode.languages.registerCompletionItemProvider('reox', {
        provideCompletionItems(document, position, token, context) {
            // Control flow keywords
            const controlFlow = [
                'if', 'else', 'while', 'for', 'in', 'match', 'when', 'return', 'break', 'continue',
                'loop', 'guard', 'defer', 'try', 'catch', 'throw', 'where'
            ];
            // Declaration keywords
            const declarations = [
                'fn', 'let', 'struct', 'kind', 'layer', 'panel', 'action', 'signal', 'emit',
                'extern', 'import', 'async', 'await', 'protocol', 'extension', 'typealias'
            ];
            // Modifiers
            const modifiers = ['mut', 'pub', 'const', 'static', 'effect'];
            // Gesture keywords (REOX-specific)
            const gestures = ['gesture', 'on_tap', 'on_pan', 'on_swipe', 'on_pinch', 'on_rotate'];
            // Other keywords
            const otherKeywords = ['self', 'super', 'as', 'bind', 'nil', 'none', 'null', 'maybe', 'true', 'false'];
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
                'slider', 'switch', 'checkbox', 'scroll', 'spacer', 'divider', 'grid',
                'color_rgba', 'color_rgb', 'color_hex', 'color_hsl',
                'color_lighten', 'color_darken', 'color_blend',
                'animate_fade_in', 'animate_fade_out', 'animate_slide_in', 'animate_slide_out',
                'animate_scale', 'animate_rotate', 'animate_pulse', 'animate_bounce'
            ];
            const allKeywords = [...controlFlow, ...declarations, ...modifiers, ...gestures, ...otherKeywords];
            const completionItems = [
                ...allKeywords.map(k => new vscode.CompletionItem(k, vscode.CompletionItemKind.Keyword)),
                ...types.map(t => new vscode.CompletionItem(t, vscode.CompletionItemKind.TypeParameter)),
                ...builtins.map(b => new vscode.CompletionItem(b, vscode.CompletionItemKind.Function))
            ];
            return completionItems;
        }
    });
    // Hover Provider with documentation
    const hoverDocs = {
        'fn': '**fn**: Declares a function.\n\n```reox\nfn name(param: type) -> return_type { }\n```',
        'async': '**async**: Defines an asynchronous function that can be awaited.',
        'await': '**await**: Pauses execution until the awaited future completes.',
        'let': '**let**: Declares an immutable variable. Use `let mut` for mutable.',
        'mut': '**mut**: Makes a variable mutable (changeable after initialization).',
        'struct': '**struct**: Defines a composite data type with named fields.',
        'kind': '**kind**: Defines an enum (REOX term for algebraic data type).',
        'layer': '**layer**: Declares a UI component container (similar to SwiftUI View).',
        'panel': '**panel**: Declares a top-level UI window.',
        'action': '**action**: Creates a closure/lambda for event handlers.',
        'signal': '**signal**: Declares a reactive signal for state management.',
        'emit': '**emit**: Emits an event to signal subscribers.',
        'guard': '**guard**: Swift-style early-exit guard statement.',
        'defer': '**defer**: Schedules code to run when scope exits.',
        'try': '**try**: Begins a try-catch exception handling block.',
        'catch': '**catch**: Handles exceptions from a try block.',
        'throw': '**throw**: Throws an exception.',
        'loop': '**loop**: Infinite loop until break.',
        'protocol': '**protocol**: Declares an interface/trait.',
        'extension': '**extension**: Adds methods to an existing type.',
        'typealias': '**typealias**: Creates a type alias.',
        'nil': '**nil**: The absence of a value (null equivalent).',
        'maybe': '**maybe**: Optional type wrapper.',
        'gesture': '**gesture**: Declares a gesture handler.',
        'on_tap': '**on_tap**: Tap gesture handler.',
        'on_pan': '**on_pan**: Pan/drag gesture handler.',
        'on_swipe': '**on_swipe**: Swipe gesture handler.',
        'on_pinch': '**on_pinch**: Pinch zoom gesture handler.',
        'on_rotate': '**on_rotate**: Rotation gesture handler.',
        'vstack': '**vstack**: Vertical stack layout container.',
        'hstack': '**hstack**: Horizontal stack layout container.',
        'zstack': '**zstack**: Z-axis layered stack container.',
    };
    const hoverProvider = vscode.languages.registerHoverProvider('reox', {
        provideHover(document, position, token) {
            const range = document.getWordRangeAtPosition(position);
            const word = document.getText(range);
            if (word && hoverDocs[word]) {
                return new vscode.Hover(new vscode.MarkdownString(hoverDocs[word]));
            }
            return undefined;
        }
    });
    context.subscriptions.push(provider, hoverProvider);
}
function deactivate() { }
//# sourceMappingURL=extension.js.map