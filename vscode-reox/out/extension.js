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
            const completionItems = [
                ...allKeywords.map(k => new vscode.CompletionItem(k, vscode.CompletionItemKind.Keyword)),
                ...types.map(t => new vscode.CompletionItem(t, vscode.CompletionItemKind.TypeParameter)),
                ...allFunctions.map(b => new vscode.CompletionItem(b, vscode.CompletionItemKind.Function))
            ];
            return completionItems;
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