/**
 * @file reox_ffi.h
 * @brief REOX Foreign Function Interface for C/C++ Integration
 * 
 * Enables REOX UI code to:
 * - Call C/C++ functions directly
 * - Export views to C structs
 * - Register callbacks for events
 * - Work with any graphics backend
 * 
 * Copyright (c) 2025 KetiveeAI - Open Source (MIT License)
 */

#ifndef REOX_FFI_H
#define REOX_FFI_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ============================================================================
 * Core Types
 * ============================================================================ */

typedef struct RxColor { uint8_t r, g, b, a; } RxColor;
typedef struct RxRect { float x, y, w, h; } RxRect;
typedef struct RxPoint { float x, y; } RxPoint;
typedef struct RxSize { float w, h; } RxSize;

/* View handle - opaque pointer to internal view */
typedef struct RxView* RxViewRef;

/* Callback types */
typedef void (*RxCallback)(void* userdata);
typedef void (*RxEventCallback)(RxViewRef view, void* event, void* userdata);
typedef void (*RxValueCallback)(RxViewRef view, double value, void* userdata);
typedef void (*RxTextCallback)(RxViewRef view, const char* text, void* userdata);

/* ============================================================================
 * Backend Interface - Implement for each graphics system
 * ============================================================================ */

typedef struct RxBackend {
    const char* name;
    
    /* Lifecycle */
    bool (*init)(int width, int height, const char* title);
    void (*destroy)(void);
    
    /* Frame */
    void (*begin_frame)(void);
    void (*end_frame)(void);
    bool (*poll_events)(void);
    
    /* Drawing primitives */
    void (*draw_rect)(RxRect rect, RxColor color, float radius);
    void (*draw_circle)(RxPoint center, float radius, RxColor color);
    void (*draw_line)(RxPoint p1, RxPoint p2, RxColor color, float width);
    void (*draw_text)(const char* text, RxPoint pos, RxColor color, float size);
    
    /* Advanced */
    void (*set_clip)(RxRect rect);
    void (*clear_clip)(void);
    void (*push_opacity)(float alpha);
    void (*pop_opacity)(void);
    
    /* Input state */
    RxPoint (*get_mouse_pos)(void);
    bool (*is_mouse_down)(int button);
    bool (*is_key_down)(int key);
    
    /* Userdata */
    void* userdata;
} RxBackend;

/* ============================================================================
 * FFI Registration - Connect C functions to REOX
 * ============================================================================ */

/* Register a C function callable from REOX */
typedef struct RxFFIFunc {
    const char* name;
    void* func_ptr;
    const char* signature;  /* e.g., "ii->v" = (int,int)->void */
} RxFFIFunc;

/* Register multiple FFI functions */
void rx_ffi_register(const RxFFIFunc* funcs, size_t count);

/* Call an FFI function by name */
int64_t rx_ffi_call(const char* name, int64_t* args, size_t argc);

/* ============================================================================
 * View Export - Get REOX views as C data
 * ============================================================================ */

/* View types */
typedef enum {
    RX_VIEW_CONTAINER = 0,
    RX_VIEW_TEXT,
    RX_VIEW_BUTTON,
    RX_VIEW_TEXTFIELD,
    RX_VIEW_SLIDER,
    RX_VIEW_TOGGLE,
    RX_VIEW_IMAGE,
    RX_VIEW_CIRCLE,
    RX_VIEW_RECT,
    RX_VIEW_CUSTOM
} RxViewType;

/* Exported view data */
typedef struct RxViewData {
    uint64_t id;
    RxViewType type;
    RxRect bounds;
    RxColor background;
    RxColor foreground;
    float corner_radius;
    float opacity;
    const char* text;
    double value;
    
    /* Hierarchy */
    struct RxViewData** children;
    size_t child_count;
    struct RxViewData* parent;
    
    /* Callbacks */
    RxCallback on_click;
    RxValueCallback on_value_change;
    RxTextCallback on_text_change;
    void* callback_userdata;
} RxViewData;

/* Export a REOX view tree to C structs */
RxViewData* rx_ffi_export_view(RxViewRef view);

/* Free exported view data */
void rx_ffi_free_view(RxViewData* data);

/* ============================================================================
 * Event System
 * ============================================================================ */

typedef enum {
    RX_EVENT_CLICK,
    RX_EVENT_HOVER,
    RX_EVENT_FOCUS,
    RX_EVENT_BLUR,
    RX_EVENT_KEY,
    RX_EVENT_TEXT,
    RX_EVENT_SCROLL,
    RX_EVENT_DRAG
} RxEventType;

typedef struct RxEvent {
    RxEventType type;
    RxPoint position;
    int key_code;
    const char* text;
    float delta_x, delta_y;
} RxEvent;

/* Dispatch event to view */
bool rx_ffi_dispatch_event(RxViewRef view, const RxEvent* event);

/* ============================================================================
 * Backend Management
 * ============================================================================ */

/* Set the active rendering backend */
void rx_ffi_set_backend(RxBackend* backend);

/* Get the active backend */
RxBackend* rx_ffi_get_backend(void);

/* Built-in backends */
extern RxBackend rx_backend_sdl;       /* SDL2 backend */
extern RxBackend rx_backend_nxrender;  /* NeolyxOS native */
extern RxBackend rx_backend_null;      /* Headless/server */

/* ============================================================================
 * App Lifecycle
 * ============================================================================ */

typedef struct RxApp {
    const char* title;
    int width;
    int height;
    RxBackend* backend;
    RxViewRef root;
    bool running;
} RxApp;

/* Create app with backend */
RxApp* rx_app_create(const char* title, int w, int h, RxBackend* backend);

/* Set root view */
void rx_app_set_root(RxApp* app, RxViewRef root);

/* Run main loop */
int rx_app_run(RxApp* app);

/* Stop running */
void rx_app_quit(RxApp* app);

/* Destroy app */
void rx_app_destroy(RxApp* app);

/* ============================================================================
 * Memory Helpers
 * ============================================================================ */

/* Allocate memory that can be freed by REOX */
void* rx_alloc(size_t size);
void* rx_realloc(void* ptr, size_t size);
void  rx_free(void* ptr);
char* rx_strdup(const char* s);

#ifdef __cplusplus
}

/* C++ convenience wrappers */
namespace rx {
    using Color = RxColor;
    using Rect = RxRect;
    using Point = RxPoint;
    using ViewRef = RxViewRef;
    
    inline Color rgb(uint8_t r, uint8_t g, uint8_t b) { return {r, g, b, 255}; }
    inline Color rgba(uint8_t r, uint8_t g, uint8_t b, uint8_t a) { return {r, g, b, a}; }
    inline Color hex(uint32_t h) { return {(uint8_t)(h>>16), (uint8_t)(h>>8), (uint8_t)h, 255}; }
}
#endif

#endif /* REOX_FFI_H */
