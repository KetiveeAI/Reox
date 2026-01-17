/**
 * @file reox_backend_sdl.c
 * @brief SDL2 Backend for REOX FFI
 * 
 * Implements RxBackend interface using SDL2.
 * Used for development and cross-platform testing.
 * 
 * Copyright (c) 2025 KetiveeAI - Open Source (MIT License)
 */

#include "reox_ffi.h"

#ifdef REOX_USE_SDL

#include <SDL2/SDL.h>
#include <math.h>

static SDL_Window* g_window = NULL;
static SDL_Renderer* g_renderer = NULL;
static int g_width = 800;
static int g_height = 600;
static float g_opacity_stack[16] = {1.0f};
static int g_opacity_idx = 0;
static RxPoint g_mouse = {0, 0};
static uint32_t g_mouse_state = 0;

/* ============================================================================
 * Backend Implementation
 * ============================================================================ */

static bool sdl_init(int width, int height, const char* title) {
    if (SDL_Init(SDL_INIT_VIDEO) < 0) {
        return false;
    }
    
    g_width = width;
    g_height = height;
    
    g_window = SDL_CreateWindow(
        title ? title : "REOX App",
        SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED,
        width, height,
        SDL_WINDOW_SHOWN | SDL_WINDOW_RESIZABLE
    );
    
    if (!g_window) return false;
    
    g_renderer = SDL_CreateRenderer(g_window, -1, 
        SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC);
    
    if (!g_renderer) {
        SDL_DestroyWindow(g_window);
        g_window = NULL;
        return false;
    }
    
    SDL_SetRenderDrawBlendMode(g_renderer, SDL_BLENDMODE_BLEND);
    return true;
}

static void sdl_destroy(void) {
    if (g_renderer) { SDL_DestroyRenderer(g_renderer); g_renderer = NULL; }
    if (g_window) { SDL_DestroyWindow(g_window); g_window = NULL; }
    SDL_Quit();
}

static void sdl_begin_frame(void) {
    SDL_SetRenderDrawColor(g_renderer, 26, 27, 38, 255);
    SDL_RenderClear(g_renderer);
    g_opacity_idx = 0;
    g_opacity_stack[0] = 1.0f;
}

static void sdl_end_frame(void) {
    SDL_RenderPresent(g_renderer);
}

static bool sdl_poll_events(void) {
    SDL_Event event;
    while (SDL_PollEvent(&event)) {
        switch (event.type) {
            case SDL_QUIT:
                return false;
            case SDL_KEYDOWN:
                if (event.key.keysym.sym == SDLK_ESCAPE) return false;
                break;
            case SDL_MOUSEMOTION:
                g_mouse.x = (float)event.motion.x;
                g_mouse.y = (float)event.motion.y;
                break;
            case SDL_MOUSEBUTTONDOWN:
            case SDL_MOUSEBUTTONUP:
                g_mouse_state = SDL_GetMouseState(NULL, NULL);
                break;
            case SDL_WINDOWEVENT:
                if (event.window.event == SDL_WINDOWEVENT_RESIZED) {
                    g_width = event.window.data1;
                    g_height = event.window.data2;
                }
                break;
        }
    }
    return true;
}

static void sdl_draw_rect(RxRect rect, RxColor color, float radius) {
    float alpha = g_opacity_stack[g_opacity_idx];
    uint8_t a = (uint8_t)(color.a * alpha);
    
    SDL_SetRenderDrawColor(g_renderer, color.r, color.g, color.b, a);
    
    if (radius <= 0) {
        SDL_FRect r = {rect.x, rect.y, rect.w, rect.h};
        SDL_RenderFillRectF(g_renderer, &r);
        return;
    }
    
    /* Rounded rect */
    float r = fminf(radius, fminf(rect.w/2, rect.h/2));
    float x = rect.x, y = rect.y, w = rect.w, h = rect.h;
    
    /* Center */
    SDL_FRect center = {x + r, y, w - 2*r, h};
    SDL_RenderFillRectF(g_renderer, &center);
    
    /* Left/Right */
    SDL_FRect left = {x, y + r, r, h - 2*r};
    SDL_FRect right = {x + w - r, y + r, r, h - 2*r};
    SDL_RenderFillRectF(g_renderer, &left);
    SDL_RenderFillRectF(g_renderer, &right);
    
    /* Corners (pixel by pixel for now) */
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

static void sdl_draw_circle(RxPoint center, float radius, RxColor color) {
    float alpha = g_opacity_stack[g_opacity_idx];
    uint8_t a = (uint8_t)(color.a * alpha);
    
    SDL_SetRenderDrawColor(g_renderer, color.r, color.g, color.b, a);
    
    int r = (int)radius;
    for (int y = -r; y <= r; y++) {
        for (int x = -r; x <= r; x++) {
            if (x*x + y*y <= r*r) {
                SDL_RenderDrawPointF(g_renderer, center.x + x, center.y + y);
            }
        }
    }
}

static void sdl_draw_line(RxPoint p1, RxPoint p2, RxColor color, float width) {
    float alpha = g_opacity_stack[g_opacity_idx];
    uint8_t a = (uint8_t)(color.a * alpha);
    
    SDL_SetRenderDrawColor(g_renderer, color.r, color.g, color.b, a);
    
    /* Simple line - SDL only has 1px lines, so we draw multiple for width */
    if (width <= 1) {
        SDL_RenderDrawLineF(g_renderer, p1.x, p1.y, p2.x, p2.y);
    } else {
        /* Perpendicular offset for thickness */
        float dx = p2.x - p1.x, dy = p2.y - p1.y;
        float len = sqrtf(dx*dx + dy*dy);
        if (len > 0) {
            float nx = -dy / len, ny = dx / len;
            for (float w = -width/2; w <= width/2; w += 1.0f) {
                SDL_RenderDrawLineF(g_renderer, 
                    p1.x + nx*w, p1.y + ny*w, 
                    p2.x + nx*w, p2.y + ny*w);
            }
        }
    }
}

static void sdl_draw_text(const char* text, RxPoint pos, RxColor color, float size) {
    /* Placeholder - SDL2 needs SDL_ttf for real text */
    float alpha = g_opacity_stack[g_opacity_idx];
    uint8_t a = (uint8_t)(color.a * alpha);
    
    SDL_SetRenderDrawColor(g_renderer, color.r, color.g, color.b, a);
    
    if (!text) return;
    
    /* Draw rectangles as placeholder for each character */
    float x = pos.x;
    float char_w = size * 0.6f;
    float char_h = size;
    
    for (const char* c = text; *c; c++) {
        if (*c != ' ') {
            SDL_FRect r = {x, pos.y, char_w - 2, char_h};
            SDL_RenderFillRectF(g_renderer, &r);
        }
        x += char_w;
    }
}

static void sdl_set_clip(RxRect rect) {
    SDL_Rect r = {(int)rect.x, (int)rect.y, (int)rect.w, (int)rect.h};
    SDL_RenderSetClipRect(g_renderer, &r);
}

static void sdl_clear_clip(void) {
    SDL_RenderSetClipRect(g_renderer, NULL);
}

static void sdl_push_opacity(float alpha) {
    if (g_opacity_idx < 15) {
        g_opacity_idx++;
        g_opacity_stack[g_opacity_idx] = g_opacity_stack[g_opacity_idx - 1] * alpha;
    }
}

static void sdl_pop_opacity(void) {
    if (g_opacity_idx > 0) g_opacity_idx--;
}

static RxPoint sdl_get_mouse(void) {
    return g_mouse;
}

static bool sdl_is_mouse_down(int button) {
    uint32_t mask = 0;
    switch (button) {
        case 0: mask = SDL_BUTTON_LMASK; break;
        case 1: mask = SDL_BUTTON_MMASK; break;
        case 2: mask = SDL_BUTTON_RMASK; break;
    }
    return (g_mouse_state & mask) != 0;
}

static bool sdl_is_key_down(int key) {
    const uint8_t* state = SDL_GetKeyboardState(NULL);
    return state[SDL_GetScancodeFromKey(key)] != 0;
}

/* Export the backend */
RxBackend rx_backend_sdl = {
    .name = "sdl2",
    .init = sdl_init,
    .destroy = sdl_destroy,
    .begin_frame = sdl_begin_frame,
    .end_frame = sdl_end_frame,
    .poll_events = sdl_poll_events,
    .draw_rect = sdl_draw_rect,
    .draw_circle = sdl_draw_circle,
    .draw_line = sdl_draw_line,
    .draw_text = sdl_draw_text,
    .set_clip = sdl_set_clip,
    .clear_clip = sdl_clear_clip,
    .push_opacity = sdl_push_opacity,
    .pop_opacity = sdl_pop_opacity,
    .get_mouse_pos = sdl_get_mouse,
    .is_mouse_down = sdl_is_mouse_down,
    .is_key_down = sdl_is_key_down,
    .userdata = NULL
};

#else

/* Stub when SDL not available */
RxBackend rx_backend_sdl = {
    .name = "sdl2-stub",
    .init = NULL,
    .destroy = NULL
};

#endif /* REOX_USE_SDL */
