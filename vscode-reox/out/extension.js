"use strict";
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = require("vscode");
const child_process_1 = require("child_process");
const fs = require("fs");
const path = require("path");
const node_1 = require("vscode-languageclient/node");
let client;
let output;
let warnedMissingServer = false;
function activate(context) {
    return __awaiter(this, void 0, void 0, function* () {
        output = vscode.window.createOutputChannel('REOX Language Server');
        context.subscriptions.push(output);
        context.subscriptions.push(vscode.languages.registerDocumentFormattingEditProvider('reox', new ReoxFormattingProvider()), vscode.commands.registerCommand('reox.formatDocument', () => __awaiter(this, void 0, void 0, function* () {
            const editor = vscode.window.activeTextEditor;
            if (!editor || editor.document.languageId !== 'reox') {
                vscode.window.showInformationMessage('Reox: open a .rx or .reox file to format it.');
                return;
            }
            yield vscode.commands.executeCommand('editor.action.formatDocument');
        })), vscode.commands.registerCommand('reox.restartLanguageServer', () => __awaiter(this, void 0, void 0, function* () {
            yield stopClient();
            warnedMissingServer = false;
            yield startClient(context);
        })), vscode.workspace.onDidChangeConfiguration((event) => __awaiter(this, void 0, void 0, function* () {
            if (event.affectsConfiguration('reox.languageServer')) {
                yield stopClient();
                warnedMissingServer = false;
                yield startClient(context);
            }
        })));
        // The static completion and hover tables are a fallback for machines
        // without reox-lsp on PATH; the language server replaces them when it runs.
        registerFallbackProviders(context);
        yield startClient(context);
    });
}
function deactivate() {
    return __awaiter(this, void 0, void 0, function* () {
        yield stopClient();
    });
}
// === Language server ===
function configuration() {
    return vscode.workspace.getConfiguration('reox');
}
/// Resolve the reox-lsp executable: the setting, else a lookup on PATH.
function resolveServerCommand() {
    var _a;
    const configured = configuration().get('languageServer.path', '').trim();
    if (configured.length > 0) {
        return configured;
    }
    const exe = process.platform === 'win32' ? 'reox-lsp.exe' : 'reox-lsp';
    const dirs = ((_a = process.env.PATH) !== null && _a !== void 0 ? _a : '').split(path.delimiter).filter((d) => d.length > 0);
    for (const dir of dirs) {
        const candidate = path.join(dir, exe);
        try {
            fs.accessSync(candidate, fs.constants.X_OK);
            return candidate;
        }
        catch (_b) {
            // keep looking
        }
    }
    return undefined;
}
function startClient(context) {
    return __awaiter(this, void 0, void 0, function* () {
        if (client) {
            return;
        }
        const config = configuration();
        if (!config.get('languageServer.enabled', true)) {
            output === null || output === void 0 ? void 0 : output.appendLine('reox.languageServer.enabled is false; not starting reox-lsp.');
            return;
        }
        const command = resolveServerCommand();
        if (!command) {
            output === null || output === void 0 ? void 0 : output.appendLine('reox-lsp not found on PATH and reox.languageServer.path is empty.');
            if (!warnedMissingServer) {
                warnedMissingServer = true;
                const choice = yield vscode.window.showWarningMessage('reox-lsp was not found. Build it with `cargo build -p reox-lsp` and put it on PATH, or set reox.languageServer.path.', 'Open Settings');
                if (choice === 'Open Settings') {
                    yield vscode.commands.executeCommand('workbench.action.openSettings', 'reox.languageServer.path');
                }
            }
            return;
        }
        const serverOptions = {
            command,
            args: ['--stdio'],
            transport: node_1.TransportKind.stdio
        };
        const initializationOptions = {};
        const prelude = config.get('languageServer.preludePath', '').trim();
        if (prelude.length > 0) {
            initializationOptions.preludePath = prelude;
        }
        const clientOptions = {
            documentSelector: [{ scheme: 'file', language: 'reox' }, { scheme: 'untitled', language: 'reox' }],
            outputChannel: output,
            initializationOptions,
            synchronize: {
                fileEvents: vscode.workspace.createFileSystemWatcher('**/*.{rx,reox}')
            }
        };
        client = new node_1.LanguageClient('reox', 'REOX Language Server', serverOptions, clientOptions);
        output === null || output === void 0 ? void 0 : output.appendLine(`Starting ${command} --stdio`);
        try {
            yield client.start();
            context.subscriptions.push({ dispose: () => { void stopClient(); } });
        }
        catch (err) {
            output === null || output === void 0 ? void 0 : output.appendLine(`Failed to start reox-lsp: ${String(err)}`);
            vscode.window.showErrorMessage(`REOX language server failed to start: ${String(err)}`);
            client = undefined;
        }
    });
}
function stopClient() {
    return __awaiter(this, void 0, void 0, function* () {
        if (!client) {
            return;
        }
        const running = client;
        client = undefined;
        try {
            yield running.stop();
        }
        catch (err) {
            output === null || output === void 0 ? void 0 : output.appendLine(`Error stopping reox-lsp: ${String(err)}`);
        }
    });
}
// === Formatter (reoxc fmt --stdin) ===
class ReoxFormattingProvider {
    provideDocumentFormattingEdits(document, _options, token) {
        const reoxc = configuration().get('reoxcPath', 'reoxc') || 'reoxc';
        const source = document.getText();
        return new Promise((resolve) => {
            var _a, _b;
            const child = (0, child_process_1.execFile)(reoxc, ['fmt', '--stdin'], { maxBuffer: 64 * 1024 * 1024 }, (error, stdout, stderr) => {
                if (token.isCancellationRequested) {
                    resolve([]);
                    return;
                }
                if (error) {
                    const detail = stderr.trim().length > 0 ? stderr.trim() : error.message;
                    output === null || output === void 0 ? void 0 : output.appendLine(`reoxc fmt failed: ${detail}`);
                    vscode.window.showErrorMessage(`Reox: format failed: ${firstLine(detail)}`);
                    resolve([]);
                    return;
                }
                if (stdout === source) {
                    resolve([]);
                    return;
                }
                const fullRange = new vscode.Range(document.positionAt(0), document.positionAt(source.length));
                resolve([vscode.TextEdit.replace(fullRange, stdout)]);
            });
            token.onCancellationRequested(() => child.kill());
            (_a = child.stdin) === null || _a === void 0 ? void 0 : _a.on('error', (err) => output === null || output === void 0 ? void 0 : output.appendLine(`reoxc fmt stdin: ${String(err)}`));
            (_b = child.stdin) === null || _b === void 0 ? void 0 : _b.end(source);
        });
    }
}
function firstLine(text) {
    const nl = text.indexOf('\n');
    return nl >= 0 ? text.slice(0, nl) : text;
}
// === Fallback providers (no language server available) ===
function registerFallbackProviders(context) {
    const provider = vscode.languages.registerCompletionItemProvider('reox', {
        provideCompletionItems() {
            if (client) {
                // The server answers completion; avoid duplicate items.
                return [];
            }
            // Control flow keywords
            const controlFlow = [
                'if', 'else', 'while', 'for', 'in', 'match', 'when', 'return', 'break', 'continue',
                'loop', 'guard', 'defer', 'try', 'catch', 'throw', 'where'
            ];
            // Declaration keywords
            const declarations = [
                'fn', 'let', 'struct', 'kind', 'layer', 'panel', 'action', 'signal', 'emit',
                'extern', 'import', 'async', 'await', 'protocol', 'extension', 'typealias', 'variant'
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
                'print', 'println', 'print_int', 'print_float', 'print_bool',
                'len', 'type_of', 'assert', 'debug', 'log'
            ];
            // Layout containers
            const layouts = [
                'vstack', 'hstack', 'zstack', 'scroll_view', 'list_view',
                'grid_view', 'flex_view', 'center', 'spacer', 'divider'
            ];
            // Basic views
            const views = [
                'text_view', 'image_view', 'icon_view', 'avatar_view',
                'badge_view', 'progress_view', 'activity_indicator',
                'video_player', 'audio_player'
            ];
            // Buttons
            const buttons = [
                'button_view', 'button_primary', 'button_secondary',
                'button_tertiary', 'button_destructive',
                'button_icon', 'button_icon_label'
            ];
            // Form inputs
            const forms = [
                'text_field', 'text_area', 'password_field', 'search_field',
                'email_field', 'number_field', 'checkbox_view', 'radio_view',
                'toggle_view', 'switch_view', 'picker_view', 'date_picker',
                'time_picker', 'color_picker', 'file_picker',
                'slider_view', 'range_slider', 'stepper_view',
                'form_view', 'form_field', 'form_section'
            ];
            // View modifiers
            const viewMods = [
                'view_set_background', 'view_set_gradient', 'view_set_foreground',
                'view_set_padding', 'view_set_margin', 'view_set_border',
                'view_set_corner_radius', 'view_set_circular', 'view_set_shadow',
                'view_set_size', 'view_set_width', 'view_set_height',
                'view_set_opacity', 'view_set_hidden', 'view_set_disabled',
                'view_add_child', 'view_remove_child', 'view_clear_children',
                'view_set_glass', 'view_set_blur', 'view_set_glow'
            ];
            // Text styling
            const textMods = [
                'text_set_text', 'text_set_color', 'text_set_font',
                'text_set_font_size', 'text_set_font_weight', 'text_set_align'
            ];
            // Colors
            const colors = [
                'color_rgba', 'color_rgb', 'color_hex',
                'color_primary', 'color_secondary', 'color_accent',
                'color_success', 'color_warning', 'color_error',
                'color_background', 'color_surface', 'color_text',
                'color_white', 'color_black', 'color_clear'
            ];
            // Geometry helpers
            const geometry = [
                'point', 'size', 'rect', 'insets', 'insets_all', 'insets_symmetric',
                'border', 'shadow', 'shadow_sm', 'shadow_md', 'shadow_lg'
            ];
            // State management
            const state = [
                'state_int', 'state_float', 'state_string', 'state_bool',
                'state_get_int', 'state_set_int', 'state_get_float', 'state_set_float',
                'state_batch_begin', 'state_batch_commit'
            ];
            // App/Window
            const app = [
                'app_new', 'app_create_window', 'app_run', 'app_quit',
                'window_set_root', 'window_set_title', 'window_set_size', 'window_center'
            ];
            // Math functions
            const math = [
                'abs', 'min', 'max', 'sqrt', 'pow', 'floor', 'ceil', 'round',
                'sin_float', 'cos_float', 'tan_float', 'clamp_int', 'clamp_float'
            ];
            // Animation
            const animation = [
                'ease_linear', 'ease_in', 'ease_out', 'ease_in_out', 'lerp',
                'set_timeout', 'set_interval', 'clear_timeout', 'request_animation_frame'
            ];
            // Shapes
            const shapes = [
                'circle', 'circle_filled', 'rounded_rect', 'capsule', 'ellipse',
                'line_view', 'triangle', 'star', 'path_view'
            ];
            const allFunctions = [
                ...builtins, ...layouts, ...views, ...buttons, ...forms,
                ...viewMods, ...textMods, ...colors, ...geometry,
                ...state, ...app, ...math, ...animation, ...shapes
            ];
            const allKeywords = [...controlFlow, ...declarations, ...modifiers, ...gestures, ...otherKeywords];
            return [
                ...allKeywords.map(k => new vscode.CompletionItem(k, vscode.CompletionItemKind.Keyword)),
                ...types.map(t => new vscode.CompletionItem(t, vscode.CompletionItemKind.TypeParameter)),
                ...allFunctions.map(b => new vscode.CompletionItem(b, vscode.CompletionItemKind.Function))
            ];
        }
    });
    // Hover Provider with documentation
    const hoverDocs = {
        // Keywords
        'fn': '**fn**: Declares a function.\n```reox\nfn name(param: type) -> return_type { }\n```',
        'async': '**async**: Defines an asynchronous function.',
        'await': '**await**: Pauses until the awaited future completes.',
        'let': '**let**: Declares a variable. Use `let mut` for mutable.',
        'mut': '**mut**: Makes a variable mutable.',
        'struct': '**struct**: Composite data type with named fields.',
        'guard': '**guard**: Early-exit if condition fails.',
        'defer': '**defer**: Runs on scope exit.',
        'match': '**match**: Pattern matching expression.',
        // Layout
        'vstack': '**vstack(gap)**: Vertical stack.\n```reox\nlet col = vstack(16.0);\n```',
        'hstack': '**hstack(gap)**: Horizontal stack.\n```reox\nlet row = hstack(8.0);\n```',
        'zstack': '**zstack()**: Overlay stack (layers on top).',
        'grid_view': '**grid_view(cols, gap)**: Grid layout.\n```reox\nlet grid = grid_view(3, 12.0);\n```',
        'scroll_view': '**scroll_view()**: Scrollable container.',
        'center': '**center()**: Centers its child view.',
        'spacer': '**spacer()**: Expands to fill space.',
        // Views
        'text_view': '**text_view(text)**: Text display.',
        'button_primary': '**button_primary(label)**: Primary button.',
        'button_secondary': '**button_secondary(label)**: Secondary button.',
        'text_field': '**text_field(placeholder)**: Text input field.',
        'slider_view': '**slider_view(min, max, value)**: Slider control.',
        // Modifiers
        'view_set_background': '**view_set_background(v, color)**: Set background color.',
        'view_set_padding': '**view_set_padding(v, insets)**: Set padding.\n```reox\nview_set_padding(card, insets_all(16.0));\n```',
        'view_set_corner_radius': '**view_set_corner_radius(v, r)**: Set corner radius.',
        'view_add_child': '**view_add_child(parent, child)**: Add child to container.',
        // Colors
        'color_primary': '**color_primary()**: Theme primary color (#007AFF).',
        'color_surface': '**color_surface()**: Surface color (#2C2C2E).',
        'color_background': '**color_background()**: Background color (#1C1C1E).',
        'color_rgba': '**color_rgba(r, g, b, a)**: Create RGBA color.',
        // Geometry
        'insets_all': '**insets_all(value)**: Equal padding all sides.',
        'insets_symmetric': '**insets_symmetric(v, h)**: Vertical, horizontal padding.',
        // App
        'app_new': '**app_new(name)**: Create application.',
        'app_create_window': '**app_create_window(app, title, w, h)**: Create window.',
        'window_set_root': '**window_set_root(win, view)**: Set root view.',
        'app_run': '**app_run(app)**: Start application event loop.',
        // State
        'state_int': '**state_int(initial)**: Reactive integer state.',
        'state_set_int': '**state_set_int(s, value)**: Update state value.',
    };
    const hoverProvider = vscode.languages.registerHoverProvider('reox', {
        provideHover(document, position) {
            if (client) {
                return undefined;
            }
            const range = document.getWordRangeAtPosition(position);
            const word = range ? document.getText(range) : '';
            if (word && hoverDocs[word]) {
                return new vscode.Hover(new vscode.MarkdownString(hoverDocs[word]));
            }
            return undefined;
        }
    });
    context.subscriptions.push(provider, hoverProvider);
}
//# sourceMappingURL=extension.js.map