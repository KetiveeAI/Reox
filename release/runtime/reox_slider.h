/*
 * REOX Slider Widget
 * Horizontal/vertical slider for value selection
 * 
 * Copyright (c) 2025 KetiveeAI
 */

#ifndef REOX_SLIDER_H
#define REOX_SLIDER_H

#include "reox_ui.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef void (*rx_slider_callback)(float value, void* user_data);

typedef struct rx_slider_view {
    rx_view base;
    
    float value;          /* 0.0 to 1.0 normalized */
    float min;            /* Minimum value */
    float max;            /* Maximum value */
    float step;           /* Increment step (0 = continuous) */
    bool vertical;        /* Vertical orientation */
    bool dragging;        /* Currently being dragged */
    
    rx_slider_callback on_change;
    void* callback_data;
    
    /* Appearance */
    rx_color track_color;
    rx_color fill_color;
    rx_color thumb_color;
    float track_height;   /* Track thickness */
    float thumb_radius;   /* Thumb circle radius */
} rx_slider_view;

/* Create slider with range */
extern rx_slider_view* slider_view_new(float min, float max, float initial);

/* Set value (clamped to range, calls callback) */
extern void slider_view_set_value(rx_slider_view* view, float value);

/* Get current value */
extern float slider_view_get_value(rx_slider_view* view);

/* Set step increment (0 for continuous) */
extern void slider_view_set_step(rx_slider_view* view, float step);

/* Set callback for value changes */
extern void slider_view_set_callback(rx_slider_view* view, rx_slider_callback cb, void* data);

/* Set orientation */
extern void slider_view_set_vertical(rx_slider_view* view, bool vertical);

/* Render slider to nxgfx context */
extern void slider_view_render(rx_slider_view* view, void* ctx);

/* Handle input event */
extern bool slider_view_handle_event(rx_slider_view* view, rx_point pos, bool pressed);

#ifdef __cplusplus
}
#endif

#endif /* REOX_SLIDER_H */
