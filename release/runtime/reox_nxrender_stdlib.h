/*
 * REOX NXRender Stdlib Wrappers
 * Maps nxrender.rx stdlib declarations to NXRender C API
 * 
 * Copyright (c) 2025-2026 KetiveeAI
 */

#ifndef REOX_NXRENDER_STDLIB_H
#define REOX_NXRENDER_STDLIB_H

#include "reox_nxrender_bridge.h"
#include "reox_ui.h"
#include <string.h>
#include <stdlib.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ============================================================================
 * NXGFX: Low-Level Graphics (from nxrender.rx)
 * ============================================================================ */

/* nxgfx_clear(color: int) -> void */
static inline void nxgfx_clear(int64_t color) {
    if (!rx_bridge || !rx_bridge->gpu) return;
    NxColor c = { 
        (uint8_t)((color >> 16) & 0xFF),
        (uint8_t)((color >> 8) & 0xFF),
        (uint8_t)(color & 0xFF),
        255
    };
    nx_gpu_clear(rx_bridge->gpu, c);
}

/* nxgfx_fill_rect(x, y, w, h, color) -> void */
static inline void nxgfx_fill_rect(int64_t x, int64_t y, int64_t w, int64_t h, int64_t color) {
    if (!rx_bridge || !rx_bridge->gpu) return;
    NxRect rect = { (float)x, (float)y, (float)w, (float)h };
    NxColor c = { 
        (uint8_t)((color >> 16) & 0xFF),
        (uint8_t)((color >> 8) & 0xFF),
        (uint8_t)(color & 0xFF),
        255
    };
    nx_gpu_fill_rect(rx_bridge->gpu, rect, c);
}

/* nxgfx_fill_rounded_rect(x, y, w, h, radius, color) -> void */
static inline void nxgfx_fill_rounded_rect(int64_t x, int64_t y, int64_t w, int64_t h,
                                            int64_t radius, int64_t color) {
    if (!rx_bridge || !rx_bridge->gpu) return;
    NxRect rect = { (float)x, (float)y, (float)w, (float)h };
    NxColor c = { 
        (uint8_t)((color >> 16) & 0xFF),
        (uint8_t)((color >> 8) & 0xFF),
        (uint8_t)(color & 0xFF),
        255
    };
    nx_gpu_fill_rounded_rect(rx_bridge->gpu, rect, c, (float)radius);
}

/* nxgfx_fill_circle(cx, cy, r, color) -> void */
static inline void nxgfx_fill_circle(int64_t cx, int64_t cy, int64_t r, int64_t color) {
    if (!rx_bridge || !rx_bridge->gpu) return;
    NxColor c = { 
        (uint8_t)((color >> 16) & 0xFF),
        (uint8_t)((color >> 8) & 0xFF),
        (uint8_t)(color & 0xFF),
        255
    };
    nx_gpu_fill_circle(rx_bridge->gpu, (float)cx, (float)cy, (float)r, c);
}

/* nxgfx_draw_text(x, y, text, color) -> void */
static inline void nxgfx_draw_text(int64_t x, int64_t y, const char* text, int64_t color) {
    if (!rx_bridge || !rx_bridge->gpu || !text) return;
    NxColor c = { 
        (uint8_t)((color >> 16) & 0xFF),
        (uint8_t)((color >> 8) & 0xFF),
        (uint8_t)(color & 0xFF),
        255
    };
    nx_gpu_draw_text(rx_bridge->gpu, text, (float)x, (float)y, c);
}

/* nxgfx_present() -> void */
static inline void nxgfx_present(void) {
    if (!rx_bridge || !rx_bridge->gpu) return;
    nx_gpu_present(rx_bridge->gpu);
}

/* ============================================================================
 * Window Management (from nxrender.rx)
 * ============================================================================ */

/* Window handle storage */
static struct {
    void* handles[64];
    int count;
} rx_window_handles = { {0}, 0 };

/* nx_window_create(title, x, y, w, h) -> int */
static inline int64_t nx_window_create_rx(const char* title, int64_t x, int64_t y, 
                                           int64_t w, int64_t h) {
    (void)x; (void)y;  /* Position handled by WM */
    rx_bridge_init((uint32_t)w, (uint32_t)h);
    rx_window_handles.handles[rx_window_handles.count] = rx_bridge;
    return ++rx_window_handles.count;
}

/* nx_window_show(handle) -> void */
static inline void nx_window_show_rx(int64_t handle) {
    (void)handle;
    /* Window always visible after creation */
}

/* nx_window_center(handle) -> void */
static inline void nx_window_center_rx(int64_t handle) {
    (void)handle;
    /* Centering handled by window manager */
}

/* ============================================================================
 * Widget Creation (from nxrender.rx)
 * ============================================================================ */

/* nx_button_create(label) -> int */
static inline int64_t nx_button_create_rx(const char* label) {
    RxNode* btn = rx_node_create(RX_NODE_BUTTON);
    if (label) {
        btn->text = strdup(label);
    }
    btn->width = 120;
    btn->height = 40;
    return (int64_t)(uintptr_t)btn;
}

/* nx_label_create(text) -> int */
static inline int64_t nx_label_create_rx(const char* text) {
    RxNode* label = rx_node_create(RX_NODE_TEXT);
    if (text) {
        label->text = strdup(text);
    }
    return (int64_t)(uintptr_t)label;
}

/* nx_textfield_create(placeholder) -> int */
static inline int64_t nx_textfield_create_rx(const char* placeholder) {
    RxNode* field = rx_node_create(RX_NODE_TEXTFIELD);
    if (placeholder) {
        field->text = strdup(placeholder);
    }
    field->width = 200;
    field->height = 36;
    return (int64_t)(uintptr_t)field;
}

/* nx_slider_create(min, max, value) -> int */
static inline int64_t nx_slider_create_rx(double min_val, double max_val, double value) {
    RxNode* slider = rx_node_create(RX_NODE_SLIDER);
    (void)min_val; (void)max_val;
    slider->value = (float)value;
    slider->width = 200;
    slider->height = 24;
    return (int64_t)(uintptr_t)slider;
}

/* nx_checkbox_create(label, checked) -> int */
static inline int64_t nx_checkbox_create_rx(const char* label, int64_t checked) {
    RxNode* cb = rx_node_create(RX_NODE_CHECKBOX);
    if (label) {
        cb->text = strdup(label);
    }
    cb->value = checked ? 1.0f : 0.0f;
    return (int64_t)(uintptr_t)cb;
}

/* nx_vstack_create(gap) -> int */
static inline int64_t nx_vstack_create_rx(int64_t gap) {
    RxNode* stack = rx_node_create(RX_NODE_VSTACK);
    stack->gap = (float)gap;
    stack->padding = 8.0f;
    return (int64_t)(uintptr_t)stack;
}

/* nx_hstack_create(gap) -> int */
static inline int64_t nx_hstack_create_rx(int64_t gap) {
    RxNode* stack = rx_node_create(RX_NODE_HSTACK);
    stack->gap = (float)gap;
    stack->padding = 8.0f;
    return (int64_t)(uintptr_t)stack;
}

/* nx_container_create() -> int */
static inline int64_t nx_container_create_rx(void) {
    RxNode* container = rx_node_create(RX_NODE_ZSTACK);
    return (int64_t)(uintptr_t)container;
}

/* ============================================================================
 * Widget Tree (from nxrender.rx)
 * ============================================================================ */

/* nx_widget_add_child(parent, child) -> void */
static inline void nx_widget_add_child_rx(int64_t parent, int64_t child) {
    RxNode* p = (RxNode*)(uintptr_t)parent;
    RxNode* c = (RxNode*)(uintptr_t)child;
    rx_node_add_child(p, c);
}

/* nx_window_set_root(window, widget) -> void */
static inline void nx_window_set_root_rx(int64_t window, int64_t widget) {
    (void)window;
    RxNode* root = (RxNode*)(uintptr_t)widget;
    if (rx_bridge) {
        rx_bridge->root = root;
        /* Initialize layout */
        root->x = 0;
        root->y = 0;
        root->width = 800;  /* Default window size */
        root->height = 600;
        rx_layout_node(root, root->width, root->height);
    }
}

/* nx_widget_set_bounds(handle, x, y, w, h) -> void */
static inline void nx_widget_set_bounds_rx(int64_t handle, int64_t x, int64_t y,
                                            int64_t w, int64_t h) {
    RxNode* node = (RxNode*)(uintptr_t)handle;
    if (node) {
        node->x = (float)x;
        node->y = (float)y;
        node->width = (float)w;
        node->height = (float)h;
        rx_invalidate(node);
    }
}

/* ============================================================================
 * Widget Properties (from nxrender.rx)
 * ============================================================================ */

/* nx_button_set_label(handle, label) -> void */
static inline void nx_button_set_label_rx(int64_t handle, const char* label) {
    RxNode* btn = (RxNode*)(uintptr_t)handle;
    if (btn) {
        if (btn->text) free(btn->text);
        btn->text = label ? strdup(label) : NULL;
        rx_invalidate(btn);
    }
}

/* nx_label_set_text(handle, text) -> void */
static inline void nx_label_set_text_rx(int64_t handle, const char* text) {
    RxNode* label = (RxNode*)(uintptr_t)handle;
    if (label) {
        if (label->text) free(label->text);
        label->text = text ? strdup(text) : NULL;
        rx_invalidate(label);
    }
}

/* nx_slider_set_value(handle, value) -> void */
static inline void nx_slider_set_value_rx(int64_t handle, double value) {
    RxNode* slider = (RxNode*)(uintptr_t)handle;
    if (slider) {
        slider->value = (float)value;
        rx_invalidate(slider);
    }
}

/* nx_slider_get_value(handle) -> float */
static inline double nx_slider_get_value_rx(int64_t handle) {
    RxNode* slider = (RxNode*)(uintptr_t)handle;
    return slider ? (double)slider->value : 0.0;
}

/* ============================================================================
 * Event Loop (from nxrender.rx)
 * ============================================================================ */

/* nx_event_loop_run() -> void */
static inline void nx_event_loop_run_rx(void) {
    if (!rx_bridge) return;
    
    /* Main loop - render frame */
    rx_frame();
}

/* nx_event_poll() -> bool */
static inline int64_t nx_event_poll_rx(void) {
    if (!rx_bridge) return 0;
    rx_frame();
    return 1;
}

/* nx_event_loop_quit() -> void */
static inline void nx_event_loop_quit_rx(void) {
    rx_bridge_destroy();
}

/* ============================================================================
 * Theme (from nxrender.rx)
 * ============================================================================ */

/* nx_theme_set(theme) -> void */
static inline void nx_theme_set_rx(int64_t theme) {
    if (!rx_bridge) return;
    if (rx_bridge->theme) {
        nx_theme_destroy(rx_bridge->theme);
    }
    rx_bridge->theme = (theme == 1) ? nx_theme_light() : nx_theme_dark();
    rx_bridge->needs_redraw = true;
}

/* nx_theme_get() -> int */
static inline int64_t nx_theme_get_rx(void) {
    /* 0 = dark, 1 = light */
    return 0;
}

/* ============================================================================
 * Screen Info (from nxrender.rx)
 * ============================================================================ */

/* nx_screen_width() -> int */
static inline int64_t nx_screen_width_rx(void) {
    return 1920;  /* Default screen width */
}

/* nx_screen_height() -> int */
static inline int64_t nx_screen_height_rx(void) {
    return 1080;  /* Default screen height */
}

/* nx_screen_scale() -> float */
static inline double nx_screen_scale_rx(void) {
    return 1.0;  /* Default 1x scale */
}

#ifdef __cplusplus
}
#endif

#endif /* REOX_NXRENDER_STDLIB_H */
