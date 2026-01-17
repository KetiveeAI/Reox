/*
 * REOX Color Picker Widget
 * HSL-based color picker with palette grid and hue slider
 * 
 * Copyright (c) 2025 KetiveeAI
 */

#ifndef REOX_COLORPICKER_H
#define REOX_COLORPICKER_H

#include "reox_ui.h"
#include "reox_slider.h"

#ifdef __cplusplus
extern "C" {
#endif

/* HSL color representation */
typedef struct rx_hsl {
    float h;    /* 0-360 hue */
    float s;    /* 0-100 saturation */
    float l;    /* 0-100 lightness */
} rx_hsl;

/* Color change callback */
typedef void (*rx_color_callback)(rx_color color, void* user_data);

/* Predefined color palette */
typedef struct rx_color_palette {
    const char* name;
    float hue;           /* Base hue for palette */
    rx_color colors[10]; /* Generated shade range */
} rx_color_palette;

/* Color picker widget */
typedef struct rx_color_picker_view {
    rx_view base;
    
    /* Current color state */
    rx_color selected_color;
    rx_hsl current_hsl;
    
    /* Callback */
    rx_color_callback on_change;
    void* callback_data;
    
    /* Configuration */
    bool show_hue_slider;
    bool show_palettes;
    bool show_preview;
    bool show_values;     /* HEX/RGB display */
    
    /* Palette configuration */
    rx_color_palette* palettes;
    size_t palette_count;
    int selected_palette;
    int selected_swatch;
    
    /* Layout */
    float swatch_size;
    float swatch_gap;
    float margin;
} rx_color_picker_view;

/* HSL <-> RGB conversion */
extern rx_color rx_hsl_to_rgb(rx_hsl hsl);
extern rx_hsl rx_rgb_to_hsl(rx_color rgb);

/* Create color picker */
extern rx_color_picker_view* color_picker_view_new(rx_color initial);

/* Destroy color picker */
extern void color_picker_view_free(rx_color_picker_view* view);

/* Get/set selected color */
extern rx_color color_picker_get_color(rx_color_picker_view* view);
extern void color_picker_set_color(rx_color_picker_view* view, rx_color color);

/* Set hue (updates palette) */
extern void color_picker_set_hue(rx_color_picker_view* view, float hue);

/* Set callback */
extern void color_picker_set_callback(rx_color_picker_view* view, 
                                       rx_color_callback cb, void* data);

/* Generate palette from hue */
extern void color_picker_generate_palette(float hue, rx_color* colors, int count);

/* Initialize default palettes */
extern void color_picker_init_default_palettes(rx_color_picker_view* view);

/* Render to nxgfx context */
extern void color_picker_view_render(rx_color_picker_view* view, void* ctx);

/* Handle input */
extern bool color_picker_view_handle_event(rx_color_picker_view* view, 
                                           rx_point pos, bool pressed);

/* Format color as string */
extern void color_picker_format_hex(rx_color color, char* buf, size_t buf_size);
extern void color_picker_format_rgb(rx_color color, char* buf, size_t buf_size);

#ifdef __cplusplus
}
#endif

#endif /* REOX_COLORPICKER_H */
