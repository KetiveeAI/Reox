/*
 * REOX SDL2 Visual Renderer (with Text Support)
 * High-level UI language to SDL2 graphics
 * SwiftUI-like declarative components
 */

#include <SDL2/SDL.h>
#include <SDL2/SDL_ttf.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>
#include <string.h>
#include <math.h>

/* UI Types from REOX */
typedef struct { int64_t r, g, b, a; } Color;
typedef struct { double top, right, bottom, left; } EdgeInsets;
typedef struct { double offset_x, offset_y, blur; Color color; } Shadow;  
typedef struct { double width; Color color; int64_t style; } Border;
typedef struct { int64_t handle; } View;
typedef struct { int64_t handle; } App;
typedef struct { int64_t handle; } Window;

#define MAX_VIEWS 1024
#define MAX_CHILDREN 64

typedef struct RenderView {
    int kind;
    char label[256];
    struct RenderView* children[MAX_CHILDREN];
    int child_count;
    float gap;
    int direction;
    struct { uint8_t r, g, b, a; } background;
    float padding;
    float corner_radius;
    struct { float x, y, blur; } shadow;
    float border_width;
    float font_size;
    struct { uint8_t r, g, b, a; } text_color;
    float width, height;
    float x, y;
    float opacity;
    bool has_size;
} RenderView;

static RenderView* g_views[MAX_VIEWS];
static int g_view_count = 0;
static SDL_Window* g_window = NULL;
static SDL_Renderer* g_renderer = NULL;
static RenderView* g_root = NULL;
static int g_win_width = 800;
static int g_win_height = 600;
static TTF_Font* g_font = NULL;
static TTF_Font* g_font_large = NULL;
static TTF_Font* g_font_small = NULL;

static View wrap_view(RenderView* v) { 
    g_views[g_view_count] = v; 
    return (View){ g_view_count++ }; 
}

static RenderView* unwrap_view(View v) { 
    return (v.handle >= 0 && v.handle < g_view_count) ? g_views[v.handle] : NULL; 
}

static RenderView* create_view(int kind, const char* label) {
    RenderView* v = (RenderView*)calloc(1, sizeof(RenderView));
    v->kind = kind;
    v->opacity = 1.0f;
    v->font_size = 14.0f;
    v->text_color = (typeof(v->text_color)){255, 255, 255, 255};
    if (label) strncpy(v->label, label, 255);
    return v;
}

/* ============================================================================ 
 * Color Functions
 * ============================================================================ */

Color color_rgba(int64_t r, int64_t g, int64_t b, int64_t a) { return (Color){r, g, b, a}; }
Color color_hex(int64_t hex) { return (Color){ (hex >> 16) & 0xFF, (hex >> 8) & 0xFF, hex & 0xFF, 255 }; }
Color color_hexa(int64_t hex, int64_t a) { return (Color){ (hex >> 16) & 0xFF, (hex >> 8) & 0xFF, hex & 0xFF, a }; }

static double hue_to_rgb(double p, double q, double t) {
    if (t < 0) t += 1; if (t > 1) t -= 1;
    if (t < 1.0/6.0) return p + (q - p) * 6.0 * t;
    if (t < 1.0/2.0) return q;
    if (t < 2.0/3.0) return p + (q - p) * (2.0/3.0 - t) * 6.0;
    return p;
}

Color color_hsl(double h, double s, double l) {
    h = fmod(h, 360.0) / 360.0; s = fmax(0, fmin(1, s)); l = fmax(0, fmin(1, l));
    double r, g, b;
    if (s == 0) { r = g = b = l; }
    else {
        double q = l < 0.5 ? l * (1 + s) : l + s - l * s;
        double p = 2 * l - q;
        r = hue_to_rgb(p, q, h + 1.0/3.0);
        g = hue_to_rgb(p, q, h);
        b = hue_to_rgb(p, q, h - 1.0/3.0);
    }
    return (Color){(int64_t)(r*255), (int64_t)(g*255), (int64_t)(b*255), 255};
}

Color color_red(void) { return (Color){239, 68, 68, 255}; }
Color color_green(void) { return (Color){34, 197, 94, 255}; }
Color color_blue(void) { return (Color){59, 130, 246, 255}; }
Color color_yellow(void) { return (Color){234, 179, 8, 255}; }
Color color_orange(void) { return (Color){249, 115, 22, 255}; }
Color color_purple(void) { return (Color){168, 85, 247, 255}; }
Color color_pink(void) { return (Color){236, 72, 153, 255}; }
Color color_cyan(void) { return (Color){6, 182, 212, 255}; }
Color color_white(void) { return (Color){255, 255, 255, 255}; }
Color color_black(void) { return (Color){0, 0, 0, 255}; }
Color color_gray(void) { return (Color){107, 114, 128, 255}; }
Color color_clear(void) { return (Color){0, 0, 0, 0}; }

Color color_lighten(Color c, double amount) {
    return (Color){ c.r + (int64_t)((255 - c.r) * amount), c.g + (int64_t)((255 - c.g) * amount), c.b + (int64_t)((255 - c.b) * amount), c.a };
}
Color color_darken(Color c, double amount) {
    return (Color){ (int64_t)(c.r * (1 - amount)), (int64_t)(c.g * (1 - amount)), (int64_t)(c.b * (1 - amount)), c.a };
}
Color color_blend(Color a, Color b, double t) {
    return (Color){ (int64_t)(a.r + (b.r - a.r) * t), (int64_t)(a.g + (b.g - a.g) * t), (int64_t)(a.b + (b.b - a.b) * t), (int64_t)(a.a + (b.a - a.a) * t) };
}

EdgeInsets insets_all(double value) { return (EdgeInsets){value, value, value, value}; }
Shadow shadow(double x, double y, double blur, Color c) { return (Shadow){x, y, blur, c}; }
Border border(double width, Color c) { return (Border){width, c, 1}; }

/* Layout Containers */
View vstack(double gap) { RenderView* v = create_view(0, "VStack"); v->direction = 0; v->gap = (float)gap; return wrap_view(v); }
View hstack(double gap) { RenderView* v = create_view(0, "HStack"); v->direction = 1; v->gap = (float)gap; return wrap_view(v); }
View zstack(void) { RenderView* v = create_view(0, "ZStack"); v->direction = 2; return wrap_view(v); }

/* Basic Views */
View text_view(const char* text) { return wrap_view(create_view(1, text)); }
View button_view(const char* label) { return wrap_view(create_view(2, label)); }
View slider_view(double min_val, double max_val, double value) { RenderView* v = create_view(7, "Slider"); v->width = 200; v->height = 20; v->has_size = true; return wrap_view(v); }
View toggle_view(const char* label, int64_t on) { return wrap_view(create_view(6, label)); }
View progress_view(double value) { RenderView* v = create_view(7, "Progress"); v->width = 200; v->height = 8; v->has_size = true; return wrap_view(v); }
View divider(void) { RenderView* v = create_view(0, "Divider"); v->height = 1; v->has_size = true; v->background = (typeof(v->background)){60, 60, 70, 255}; return wrap_view(v); }
View spacer(void) { return wrap_view(create_view(0, "Spacer")); }

/* Shape Views */
View circle_view(double radius) { RenderView* v = create_view(10, "Circle"); v->width = v->height = (float)(radius * 2); v->corner_radius = (float)radius; v->has_size = true; return wrap_view(v); }
View ellipse_view(double w, double h) { RenderView* v = create_view(11, "Ellipse"); v->width = (float)w; v->height = (float)h; v->corner_radius = (float)(fmin(w, h) / 2); v->has_size = true; return wrap_view(v); }
View capsule_view(double w, double h) { RenderView* v = create_view(12, "Capsule"); v->width = (float)w; v->height = (float)h; v->corner_radius = (float)(fmin(w, h) / 2); v->has_size = true; return wrap_view(v); }
View rounded_rect_view(double radius) { RenderView* v = create_view(13, "RoundedRect"); v->corner_radius = (float)radius; return wrap_view(v); }

/* View Modifiers */
void view_set_background(View v, Color c) { RenderView* vw = unwrap_view(v); if (vw) { vw->background.r = c.r; vw->background.g = c.g; vw->background.b = c.b; vw->background.a = c.a; } }
void view_set_padding(View v, EdgeInsets p) { RenderView* vw = unwrap_view(v); if (vw) vw->padding = (float)p.top; }
void view_set_corner_radius(View v, double r) { RenderView* vw = unwrap_view(v); if (vw) vw->corner_radius = (float)r; }
void view_set_shadow(View v, Shadow s) { RenderView* vw = unwrap_view(v); if (vw) { vw->shadow.x = s.offset_x; vw->shadow.y = s.offset_y; vw->shadow.blur = s.blur; } }
void view_set_border(View v, Border b) { RenderView* vw = unwrap_view(v); if (vw) vw->border_width = (float)b.width; }
void view_set_size(View v, double w, double h) { RenderView* vw = unwrap_view(v); if (vw) { vw->width = (float)w; vw->height = (float)h; vw->has_size = true; } }
void view_set_opacity(View v, double alpha) { RenderView* vw = unwrap_view(v); if (vw) vw->opacity = (float)alpha; }
void view_add_child(View parent, View child) { RenderView* p = unwrap_view(parent); RenderView* c = unwrap_view(child); if (p && c && p->child_count < MAX_CHILDREN) p->children[p->child_count++] = c; }
void text_set_font_size(View v, double size) { RenderView* vw = unwrap_view(v); if (vw) vw->font_size = (float)size; }
void text_set_color(View v, Color c) { RenderView* vw = unwrap_view(v); if (vw) { vw->text_color.r = c.r; vw->text_color.g = c.g; vw->text_color.b = c.b; vw->text_color.a = c.a; } }

/* App & Window */
App app_new(const char* name) {
    if (SDL_Init(SDL_INIT_VIDEO) < 0) { fprintf(stderr, "SDL Init failed\n"); exit(1); }
    if (TTF_Init() < 0) { fprintf(stderr, "TTF Init failed\n"); exit(1); }
    
    /* Try to load a font */
    const char* font_paths[] = {
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        NULL
    };
    
    for (int i = 0; font_paths[i]; i++) {
        g_font = TTF_OpenFont(font_paths[i], 14);
        if (g_font) {
            g_font_large = TTF_OpenFont(font_paths[i], 24);
            g_font_small = TTF_OpenFont(font_paths[i], 12);
            break;
        }
    }
    
    if (!g_font) fprintf(stderr, "Warning: No font found, text will not render\n");
    
    return (App){0};
}

Window app_create_window(App a, const char* title, int64_t w, int64_t h) {
    (void)a;
    g_win_width = (int)w; g_win_height = (int)h;
    g_window = SDL_CreateWindow(title, SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED,
        g_win_width, g_win_height, SDL_WINDOW_SHOWN | SDL_WINDOW_RESIZABLE);
    g_renderer = SDL_CreateRenderer(g_window, -1, SDL_RENDERER_ACCELERATED);
    SDL_SetRenderDrawBlendMode(g_renderer, SDL_BLENDMODE_BLEND);
    return (Window){0};
}

void window_set_root(Window win, View v) { (void)win; g_root = unwrap_view(v); }

/* Layout */
static void layout_view(RenderView* v, float x, float y, float w, float h) {
    if (!v) return;
    v->x = x; v->y = y;
    if (v->has_size) { w = v->width > 0 ? v->width : w; h = v->height > 0 ? v->height : h; }
    v->width = w; v->height = h;
    float px = v->padding, cx = x + px, cy = y + px, cw = w - 2*px, ch = h - 2*px;
    if (v->child_count == 0) return;
    if (v->direction == 0) {
        float total_gap = v->gap * (v->child_count - 1);
        float child_h = (ch - total_gap) / v->child_count;
        float yy = cy;
        for (int i = 0; i < v->child_count; i++) { layout_view(v->children[i], cx, yy, cw, child_h); yy += child_h + v->gap; }
    } else if (v->direction == 1) {
        float total_gap = v->gap * (v->child_count - 1);
        float child_w = (cw - total_gap) / v->child_count;
        float xx = cx;
        for (int i = 0; i < v->child_count; i++) { layout_view(v->children[i], xx, cy, child_w, ch); xx += child_w + v->gap; }
    } else {
        for (int i = 0; i < v->child_count; i++) layout_view(v->children[i], cx, cy, cw, ch);
    }
}

/* Drawing */
static void draw_circle_filled(float cx, float cy, float r, uint8_t cr, uint8_t cg, uint8_t cb, uint8_t ca) {
    SDL_SetRenderDrawColor(g_renderer, cr, cg, cb, ca);
    int ri = (int)r;
    for (int y = -ri; y <= ri; y++) {
        for (int x = -ri; x <= ri; x++) {
            if (x*x + y*y <= ri*ri) SDL_RenderDrawPointF(g_renderer, cx + x, cy + y);
        }
    }
}

static void draw_rounded_rect(float x, float y, float w, float h, float r, uint8_t cr, uint8_t cg, uint8_t cb, uint8_t ca) {
    SDL_SetRenderDrawColor(g_renderer, cr, cg, cb, ca);
    if (r <= 0) { SDL_FRect rect = {x, y, w, h}; SDL_RenderFillRectF(g_renderer, &rect); return; }
    r = fminf(r, fminf(w/2, h/2));
    SDL_FRect center = {x + r, y, w - 2*r, h}; SDL_RenderFillRectF(g_renderer, &center);
    SDL_FRect left = {x, y + r, r, h - 2*r}; SDL_RenderFillRectF(g_renderer, &left);
    SDL_FRect right = {x + w - r, y + r, r, h - 2*r}; SDL_RenderFillRectF(g_renderer, &right);
    for (int cy = 0; cy < (int)r; cy++) {
        for (int cx = 0; cx < (int)r; cx++) {
            float dx = r - cx - 0.5f, dy = r - cy - 0.5f;
            if (dx*dx + dy*dy <= r*r) {
                SDL_RenderDrawPointF(g_renderer, x + cx, y + cy);
                SDL_RenderDrawPointF(g_renderer, x + w - r + cx, y + cy);
                SDL_RenderDrawPointF(g_renderer, x + cx, y + h - r + cy);
                SDL_RenderDrawPointF(g_renderer, x + w - r + cx, y + h - r + cy);
            }
        }
    }
}

static void draw_text(const char* text, float x, float y, float size, uint8_t r, uint8_t g, uint8_t b, uint8_t a) {
    if (!text || !text[0]) return;
    
    TTF_Font* font = g_font;
    if (size >= 20) font = g_font_large;
    else if (size <= 12) font = g_font_small;
    
    if (!font) {
        /* Fallback: draw rectangle for each char */
        SDL_SetRenderDrawColor(g_renderer, r, g, b, a);
        float char_w = size * 0.6f;
        float xx = x;
        for (const char* c = text; *c; c++) {
            if (*c != ' ') {
                SDL_FRect rect = {xx, y, char_w - 2, size};
                SDL_RenderFillRectF(g_renderer, &rect);
            }
            xx += char_w;
        }
        return;
    }
    
    SDL_Color color = {r, g, b, a};
    SDL_Surface* surface = TTF_RenderUTF8_Blended(font, text, color);
    if (!surface) return;
    
    SDL_Texture* texture = SDL_CreateTextureFromSurface(g_renderer, surface);
    if (texture) {
        SDL_FRect dst = {x, y, (float)surface->w, (float)surface->h};
        SDL_RenderCopyF(g_renderer, texture, NULL, &dst);
        SDL_DestroyTexture(texture);
    }
    SDL_FreeSurface(surface);
}

static void render_view(RenderView* v) {
    if (!v || v->opacity <= 0) return;
    float x = v->x, y = v->y, w = v->width, h = v->height;
    
    /* Shadow */
    if (v->shadow.blur > 0)
        draw_rounded_rect(x + v->shadow.x + 2, y + v->shadow.y + 2, w, h, v->corner_radius, 0, 0, 0, 60);
    
    /* Background */
    if (v->background.a > 0) {
        if (v->kind == 10) { /* Circle */
            draw_circle_filled(x + w/2, y + h/2, w/2, v->background.r, v->background.g, v->background.b, v->background.a);
        } else {
            draw_rounded_rect(x, y, w, h, v->corner_radius, v->background.r, v->background.g, v->background.b, v->background.a);
        }
    }
    
    /* Text */
    if (v->kind == 1 && v->label[0]) {
        draw_text(v->label, x + v->padding + 4, y + v->padding + (h - v->font_size)/2, v->font_size,
            v->text_color.r, v->text_color.g, v->text_color.b, v->text_color.a);
    }
    
    /* Button */
    if (v->kind == 2 && v->label[0]) {
        int tw = 0, th = 0;
        TTF_Font* font = g_font;
        if (font && TTF_SizeUTF8(font, v->label, &tw, &th) == 0) {
            draw_text(v->label, x + (w - tw)/2, y + (h - th)/2, 14, 255, 255, 255, 255);
        } else {
            draw_text(v->label, x + v->padding + 8, y + (h - 14)/2, 14, 255, 255, 255, 255);
        }
    }
    
    for (int i = 0; i < v->child_count; i++) render_view(v->children[i]);
}

void app_run(App a) {
    (void)a;
    if (!g_root || !g_window) return;
    printf("[REOX] Visual render started\n");
    layout_view(g_root, 0, 0, (float)g_win_width, (float)g_win_height);
    bool running = true;
    SDL_Event event;
    while (running) {
        while (SDL_PollEvent(&event)) {
            if (event.type == SDL_QUIT || (event.type == SDL_KEYDOWN && event.key.keysym.sym == SDLK_ESCAPE)) running = false;
            if (event.type == SDL_WINDOWEVENT && event.window.event == SDL_WINDOWEVENT_RESIZED) {
                g_win_width = event.window.data1; g_win_height = event.window.data2;
                layout_view(g_root, 0, 0, (float)g_win_width, (float)g_win_height);
            }
        }
        SDL_SetRenderDrawColor(g_renderer, 26, 27, 38, 255);
        SDL_RenderClear(g_renderer);
        render_view(g_root);
        SDL_RenderPresent(g_renderer);
        SDL_Delay(16);
    }
    if (g_font) TTF_CloseFont(g_font);
    if (g_font_large) TTF_CloseFont(g_font_large);
    if (g_font_small) TTF_CloseFont(g_font_small);
    TTF_Quit();
    SDL_DestroyRenderer(g_renderer);
    SDL_DestroyWindow(g_window);
    SDL_Quit();
}

void println(const char* s) { printf("%s\n", s); }
void print(const char* s) { printf("%s", s); }
