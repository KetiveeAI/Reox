/**
 * @file reox_ffi.c
 * @brief REOX FFI Implementation
 * 
 * Copyright (c) 2025 KetiveeAI - Open Source (MIT License)
 */

#include "reox_ffi.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

/* ============================================================================
 * FFI Function Registry
 * ============================================================================ */

#define MAX_FFI_FUNCS 256

static struct {
    RxFFIFunc funcs[MAX_FFI_FUNCS];
    size_t count;
} g_ffi_registry = {0};

void rx_ffi_register(const RxFFIFunc* funcs, size_t count) {
    for (size_t i = 0; i < count && g_ffi_registry.count < MAX_FFI_FUNCS; i++) {
        g_ffi_registry.funcs[g_ffi_registry.count++] = funcs[i];
    }
}

static RxFFIFunc* rx_ffi_find(const char* name) {
    for (size_t i = 0; i < g_ffi_registry.count; i++) {
        if (strcmp(g_ffi_registry.funcs[i].name, name) == 0) {
            return &g_ffi_registry.funcs[i];
        }
    }
    return NULL;
}

int64_t rx_ffi_call(const char* name, int64_t* args, size_t argc) {
    RxFFIFunc* f = rx_ffi_find(name);
    if (!f || !f->func_ptr) return 0;
    
    /* Call based on signature */
    typedef int64_t (*fn0)(void);
    typedef int64_t (*fn1)(int64_t);
    typedef int64_t (*fn2)(int64_t,int64_t);
    typedef int64_t (*fn3)(int64_t,int64_t,int64_t);
    typedef int64_t (*fn4)(int64_t,int64_t,int64_t,int64_t);
    
    switch (argc) {
        case 0: return ((fn0)f->func_ptr)();
        case 1: return ((fn1)f->func_ptr)(args[0]);
        case 2: return ((fn2)f->func_ptr)(args[0], args[1]);
        case 3: return ((fn3)f->func_ptr)(args[0], args[1], args[2]);
        case 4: return ((fn4)f->func_ptr)(args[0], args[1], args[2], args[3]);
        default: return 0;
    }
}

/* ============================================================================
 * Backend Management
 * ============================================================================ */

static RxBackend* g_backend = NULL;

void rx_ffi_set_backend(RxBackend* backend) {
    g_backend = backend;
}

RxBackend* rx_ffi_get_backend(void) {
    return g_backend;
}

/* ============================================================================
 * View Export
 * ============================================================================ */

RxViewData* rx_ffi_export_view(RxViewRef view) {
    if (!view) return NULL;
    
    RxViewData* data = (RxViewData*)calloc(1, sizeof(RxViewData));
    /* Copy from internal view structure - this would hook into the actual view system */
    data->type = RX_VIEW_CONTAINER;
    data->bounds = (RxRect){0, 0, 100, 100};
    data->background = (RxColor){255, 255, 255, 255};
    data->opacity = 1.0;
    
    return data;
}

void rx_ffi_free_view(RxViewData* data) {
    if (!data) return;
    for (size_t i = 0; i < data->child_count; i++) {
        rx_ffi_free_view(data->children[i]);
    }
    free(data->children);
    free(data);
}

/* ============================================================================
 * Event Dispatch
 * ============================================================================ */

bool rx_ffi_dispatch_event(RxViewRef view, const RxEvent* event) {
    if (!view || !event) return false;
    /* Hook into view's event handlers */
    return true;
}

/* ============================================================================
 * App Lifecycle
 * ============================================================================ */

RxApp* rx_app_create(const char* title, int w, int h, RxBackend* backend) {
    RxApp* app = (RxApp*)calloc(1, sizeof(RxApp));
    app->title = rx_strdup(title);
    app->width = w;
    app->height = h;
    app->backend = backend;
    app->running = false;
    
    if (backend && backend->init) {
        backend->init(w, h, title);
    }
    
    rx_ffi_set_backend(backend);
    return app;
}

void rx_app_set_root(RxApp* app, RxViewRef root) {
    if (app) app->root = root;
}

int rx_app_run(RxApp* app) {
    if (!app || !app->backend) return -1;
    
    app->running = true;
    RxBackend* b = app->backend;
    
    while (app->running) {
        if (b->poll_events && !b->poll_events()) {
            app->running = false;
            break;
        }
        
        if (b->begin_frame) b->begin_frame();
        
        /* Render root view tree */
        /* TODO: Hook into actual view rendering */
        
        if (b->end_frame) b->end_frame();
    }
    
    return 0;
}

void rx_app_quit(RxApp* app) {
    if (app) app->running = false;
}

void rx_app_destroy(RxApp* app) {
    if (!app) return;
    
    if (app->backend && app->backend->destroy) {
        app->backend->destroy();
    }
    
    free((void*)app->title);
    free(app);
}

/* ============================================================================
 * Memory Helpers
 * ============================================================================ */

void* rx_alloc(size_t size) {
    return calloc(1, size);
}

void* rx_realloc(void* ptr, size_t size) {
    return realloc(ptr, size);
}

void rx_free(void* ptr) {
    free(ptr);
}

char* rx_strdup(const char* s) {
    if (!s) return NULL;
    size_t len = strlen(s);
    char* copy = (char*)malloc(len + 1);
    if (copy) memcpy(copy, s, len + 1);
    return copy;
}

/* ============================================================================
 * Null Backend (for server-side / headless)
 * ============================================================================ */

static bool null_init(int w, int h, const char* t) { (void)w; (void)h; (void)t; return true; }
static void null_destroy(void) {}
static void null_begin(void) {}
static void null_end(void) {}
static bool null_poll(void) { return true; }
static void null_rect(RxRect r, RxColor c, float rad) { (void)r; (void)c; (void)rad; }
static void null_circle(RxPoint p, float r, RxColor c) { (void)p; (void)r; (void)c; }
static void null_line(RxPoint a, RxPoint b, RxColor c, float w) { (void)a; (void)b; (void)c; (void)w; }
static void null_text(const char* t, RxPoint p, RxColor c, float s) { (void)t; (void)p; (void)c; (void)s; }
static RxPoint null_mouse(void) { return (RxPoint){0,0}; }
static bool null_mousedown(int b) { (void)b; return false; }
static bool null_keydown(int k) { (void)k; return false; }

RxBackend rx_backend_null = {
    .name = "null",
    .init = null_init,
    .destroy = null_destroy,
    .begin_frame = null_begin,
    .end_frame = null_end,
    .poll_events = null_poll,
    .draw_rect = null_rect,
    .draw_circle = null_circle,
    .draw_line = null_line,
    .draw_text = null_text,
    .get_mouse_pos = null_mouse,
    .is_mouse_down = null_mousedown,
    .is_key_down = null_keydown,
    .userdata = NULL
};
