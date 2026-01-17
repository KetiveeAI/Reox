/*
 * REOX Slider Widget Implementation
 * Horizontal/vertical slider for value selection
 * 
 * Copyright (c) 2025 KetiveeAI
 */

#include "reox_slider.h"
#include "nxgfx/nxgfx.h"
#include <stdlib.h>
#include <math.h>

static float clamp(float v, float min, float max) {
    return v < min ? min : (v > max ? max : v);
}

static float normalize(float value, float min, float max) {
    if (max == min) return 0.0f;
    return (value - min) / (max - min);
}

static float denormalize(float t, float min, float max) {
    return min + t * (max - min);
}

rx_slider_view* slider_view_new(float min, float max, float initial) {
    rx_slider_view* slider = calloc(1, sizeof(rx_slider_view));
    if (!slider) return NULL;
    
    slider->base.kind = RX_VIEW_CUSTOM;
    slider->base.visible = true;
    slider->base.enabled = true;
    
    slider->min = min;
    slider->max = max;
    slider->value = clamp(initial, min, max);
    slider->step = 0.0f;
    slider->vertical = false;
    slider->dragging = false;
    
    /* Default appearance */
    slider->track_color = color_hex(0x3A3A3C);
    slider->fill_color = color_hex(0x007AFF);
    slider->thumb_color = RX_COLOR_WHITE;
    slider->track_height = 4.0f;
    slider->thumb_radius = 10.0f;
    
    /* Default size */
    slider->base.box.width = 200;
    slider->base.box.height = 24;
    
    return slider;
}

void slider_view_set_value(rx_slider_view* view, float value) {
    if (!view) return;
    
    float new_val = clamp(value, view->min, view->max);
    
    /* Apply step if set */
    if (view->step > 0.0f) {
        new_val = roundf(new_val / view->step) * view->step;
        new_val = clamp(new_val, view->min, view->max);
    }
    
    if (new_val != view->value) {
        view->value = new_val;
        if (view->on_change) {
            view->on_change(new_val, view->callback_data);
        }
    }
}

float slider_view_get_value(rx_slider_view* view) {
    return view ? view->value : 0.0f;
}

void slider_view_set_step(rx_slider_view* view, float step) {
    if (view) view->step = step > 0.0f ? step : 0.0f;
}

void slider_view_set_callback(rx_slider_view* view, rx_slider_callback cb, void* data) {
    if (!view) return;
    view->on_change = cb;
    view->callback_data = data;
}

void slider_view_set_vertical(rx_slider_view* view, bool vertical) {
    if (view) view->vertical = vertical;
}

void slider_view_render(rx_slider_view* view, void* ctx) {
    if (!view || !ctx || !view->base.visible) return;
    
    nx_context_t* gfx = (nx_context_t*)ctx;
    rx_rect frame = view->base.box.frame;
    float t = normalize(view->value, view->min, view->max);
    
    if (view->vertical) {
        /* Vertical slider */
        float center_x = frame.x + frame.width / 2;
        float track_top = frame.y + view->thumb_radius;
        float track_bottom = frame.y + frame.height - view->thumb_radius;
        float track_len = track_bottom - track_top;
        
        /* Track background */
        nx_rect_t track_rect = {
            (int32_t)(center_x - view->track_height / 2),
            (int32_t)track_top,
            (uint32_t)view->track_height,
            (uint32_t)track_len
        };
        nx_color_t track_c = {view->track_color.r, view->track_color.g, 
                              view->track_color.b, view->track_color.a};
        nxgfx_fill_rounded_rect(gfx, track_rect, track_c, 
                                (uint32_t)(view->track_height / 2));
        
        /* Filled portion */
        float fill_y = track_top + track_len * (1 - t);
        nx_rect_t fill_rect = {
            (int32_t)(center_x - view->track_height / 2),
            (int32_t)fill_y,
            (uint32_t)view->track_height,
            (uint32_t)(track_len * t)
        };
        nx_color_t fill_c = {view->fill_color.r, view->fill_color.g,
                             view->fill_color.b, view->fill_color.a};
        nxgfx_fill_rounded_rect(gfx, fill_rect, fill_c,
                                (uint32_t)(view->track_height / 2));
        
        /* Thumb */
        float thumb_y = track_top + track_len * (1 - t);
        nx_point_t thumb_center = {(int32_t)center_x, (int32_t)thumb_y};
        nx_color_t thumb_c = {view->thumb_color.r, view->thumb_color.g,
                              view->thumb_color.b, view->thumb_color.a};
        nxgfx_fill_circle(gfx, thumb_center, (uint32_t)view->thumb_radius, thumb_c);
    } else {
        /* Horizontal slider */
        float center_y = frame.y + frame.height / 2;
        float track_left = frame.x + view->thumb_radius;
        float track_right = frame.x + frame.width - view->thumb_radius;
        float track_len = track_right - track_left;
        
        /* Track background */
        nx_rect_t track_rect = {
            (int32_t)track_left,
            (int32_t)(center_y - view->track_height / 2),
            (uint32_t)track_len,
            (uint32_t)view->track_height
        };
        nx_color_t track_c = {view->track_color.r, view->track_color.g,
                              view->track_color.b, view->track_color.a};
        nxgfx_fill_rounded_rect(gfx, track_rect, track_c,
                                (uint32_t)(view->track_height / 2));
        
        /* Filled portion */
        nx_rect_t fill_rect = {
            (int32_t)track_left,
            (int32_t)(center_y - view->track_height / 2),
            (uint32_t)(track_len * t),
            (uint32_t)view->track_height
        };
        nx_color_t fill_c = {view->fill_color.r, view->fill_color.g,
                             view->fill_color.b, view->fill_color.a};
        nxgfx_fill_rounded_rect(gfx, fill_rect, fill_c,
                                (uint32_t)(view->track_height / 2));
        
        /* Thumb */
        float thumb_x = track_left + track_len * t;
        nx_point_t thumb_center = {(int32_t)thumb_x, (int32_t)center_y};
        nx_color_t thumb_c = {view->thumb_color.r, view->thumb_color.g,
                              view->thumb_color.b, view->thumb_color.a};
        nxgfx_fill_circle(gfx, thumb_center, (uint32_t)view->thumb_radius, thumb_c);
    }
}

bool slider_view_handle_event(rx_slider_view* view, rx_point pos, bool pressed) {
    if (!view || !view->base.enabled) return false;
    
    rx_rect frame = view->base.box.frame;
    
    /* Check if point is in slider bounds */
    bool in_bounds = pos.x >= frame.x && pos.x < frame.x + frame.width &&
                     pos.y >= frame.y && pos.y < frame.y + frame.height;
    
    if (pressed && (in_bounds || view->dragging)) {
        view->dragging = true;
        
        /* Calculate new value from position */
        float t;
        if (view->vertical) {
            float track_top = frame.y + view->thumb_radius;
            float track_bottom = frame.y + frame.height - view->thumb_radius;
            t = 1.0f - clamp((pos.y - track_top) / (track_bottom - track_top), 0.0f, 1.0f);
        } else {
            float track_left = frame.x + view->thumb_radius;
            float track_right = frame.x + frame.width - view->thumb_radius;
            t = clamp((pos.x - track_left) / (track_right - track_left), 0.0f, 1.0f);
        }
        
        float new_val = denormalize(t, view->min, view->max);
        slider_view_set_value(view, new_val);
        return true;
    } else {
        view->dragging = false;
    }
    
    return false;
}
