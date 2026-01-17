/*
 * REOX Color Picker Widget Implementation
 * HSL-based color picker with palette grid and hue slider
 * 
 * Copyright (c) 2025 KetiveeAI
 */

#include "reox_colorpicker.h"
#include "nxgfx/nxgfx.h"
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <math.h>

static float clamp_f(float v, float min, float max) {
    return v < min ? min : (v > max ? max : v);
}

static float hue_to_rgb_component(float p, float q, float t) {
    if (t < 0.0f) t += 1.0f;
    if (t > 1.0f) t -= 1.0f;
    if (t < 1.0f/6.0f) return p + (q - p) * 6.0f * t;
    if (t < 1.0f/2.0f) return q;
    if (t < 2.0f/3.0f) return p + (q - p) * (2.0f/3.0f - t) * 6.0f;
    return p;
}

rx_color rx_hsl_to_rgb(rx_hsl hsl) {
    float h = hsl.h / 360.0f;
    float s = hsl.s / 100.0f;
    float l = hsl.l / 100.0f;
    
    rx_color rgb = {0, 0, 0, 255};
    
    if (s == 0.0f) {
        uint8_t gray = (uint8_t)(l * 255.0f);
        rgb.r = rgb.g = rgb.b = gray;
    } else {
        float q = l < 0.5f ? l * (1.0f + s) : l + s - l * s;
        float p = 2.0f * l - q;
        
        rgb.r = (uint8_t)(hue_to_rgb_component(p, q, h + 1.0f/3.0f) * 255.0f);
        rgb.g = (uint8_t)(hue_to_rgb_component(p, q, h) * 255.0f);
        rgb.b = (uint8_t)(hue_to_rgb_component(p, q, h - 1.0f/3.0f) * 255.0f);
    }
    
    return rgb;
}

rx_hsl rx_rgb_to_hsl(rx_color rgb) {
    float r = rgb.r / 255.0f;
    float g = rgb.g / 255.0f;
    float b = rgb.b / 255.0f;
    
    float max = fmaxf(fmaxf(r, g), b);
    float min = fminf(fminf(r, g), b);
    float delta = max - min;
    
    rx_hsl hsl = {0, 0, 0};
    hsl.l = (max + min) / 2.0f * 100.0f;
    
    if (delta == 0.0f) {
        hsl.h = 0.0f;
        hsl.s = 0.0f;
    } else {
        hsl.s = (hsl.l > 50.0f ? delta / (2.0f - max - min) : delta / (max + min)) * 100.0f;
        
        if (max == r) {
            hsl.h = 60.0f * fmodf((g - b) / delta, 6.0f);
        } else if (max == g) {
            hsl.h = 60.0f * ((b - r) / delta + 2.0f);
        } else {
            hsl.h = 60.0f * ((r - g) / delta + 4.0f);
        }
        
        if (hsl.h < 0.0f) hsl.h += 360.0f;
    }
    
    return hsl;
}

void color_picker_generate_palette(float hue, rx_color* colors, int count) {
    float saturations[] = {95, 90, 85, 75, 65, 55, 45, 35, 25, 15};
    float lightnesses[] = {95, 88, 78, 68, 58, 48, 38, 28, 20, 12};
    
    for (int i = 0; i < count && i < 10; i++) {
        rx_hsl hsl = {hue, saturations[i], lightnesses[i]};
        colors[i] = rx_hsl_to_rgb(hsl);
    }
}

void color_picker_init_default_palettes(rx_color_picker_view* view) {
    if (!view) return;
    
    static const struct { const char* name; float hue; } defaults[] = {
        {"Blues",   220.0f},
        {"Purples", 270.0f},
        {"Pinks",   330.0f},
        {"Reds",    0.0f},
        {"Oranges", 25.0f},
        {"Yellows", 50.0f},
        {"Greens",  140.0f},
        {"Teals",   180.0f},
    };
    
    view->palette_count = 9;
    view->palettes = calloc(9, sizeof(rx_color_palette));
    
    for (size_t i = 0; i < 8; i++) {
        view->palettes[i].name = defaults[i].name;
        view->palettes[i].hue = defaults[i].hue;
        color_picker_generate_palette(defaults[i].hue, view->palettes[i].colors, 10);
    }
    
    /* Grays palette */
    view->palettes[8].name = "Grays";
    view->palettes[8].hue = 0.0f;
    for (int i = 0; i < 10; i++) {
        uint8_t gray = (uint8_t)(242 - i * 23);
        view->palettes[8].colors[i] = color_rgba(gray, gray, gray, 255);
    }
}

rx_color_picker_view* color_picker_view_new(rx_color initial) {
    rx_color_picker_view* picker = calloc(1, sizeof(rx_color_picker_view));
    if (!picker) return NULL;
    
    picker->base.kind = RX_VIEW_CUSTOM;
    picker->base.visible = true;
    picker->base.enabled = true;
    
    picker->selected_color = initial;
    picker->current_hsl = rx_rgb_to_hsl(initial);
    
    picker->show_hue_slider = true;
    picker->show_palettes = true;
    picker->show_preview = true;
    picker->show_values = true;
    
    picker->swatch_size = 40.0f;
    picker->swatch_gap = 8.0f;
    picker->margin = 24.0f;
    
    picker->selected_palette = -1;
    picker->selected_swatch = -1;
    
    color_picker_init_default_palettes(picker);
    
    return picker;
}

void color_picker_view_free(rx_color_picker_view* view) {
    if (!view) return;
    free(view->palettes);
    free(view);
}

rx_color color_picker_get_color(rx_color_picker_view* view) {
    return view ? view->selected_color : RX_COLOR_BLACK;
}

void color_picker_set_color(rx_color_picker_view* view, rx_color color) {
    if (!view) return;
    view->selected_color = color;
    view->current_hsl = rx_rgb_to_hsl(color);
    if (view->on_change) {
        view->on_change(color, view->callback_data);
    }
}

void color_picker_set_hue(rx_color_picker_view* view, float hue) {
    if (!view) return;
    view->current_hsl.h = clamp_f(hue, 0.0f, 360.0f);
    view->selected_color = rx_hsl_to_rgb(view->current_hsl);
    if (view->on_change) {
        view->on_change(view->selected_color, view->callback_data);
    }
}

void color_picker_set_callback(rx_color_picker_view* view, 
                                rx_color_callback cb, void* data) {
    if (!view) return;
    view->on_change = cb;
    view->callback_data = data;
}

void color_picker_format_hex(rx_color color, char* buf, size_t buf_size) {
    snprintf(buf, buf_size, "#%02X%02X%02X", color.r, color.g, color.b);
}

void color_picker_format_rgb(rx_color color, char* buf, size_t buf_size) {
    snprintf(buf, buf_size, "rgb(%d, %d, %d)", color.r, color.g, color.b);
}

static void render_rainbow_slider(nx_context_t* ctx, rx_rect frame, float current_hue) {
    float width = frame.width;
    
    /* Draw rainbow gradient bar */
    for (int x = 0; x < (int)width; x++) {
        float hue = (float)x / width * 360.0f;
        rx_hsl hsl = {hue, 100.0f, 50.0f};
        rx_color col = rx_hsl_to_rgb(hsl);
        
        nx_rect_t seg = {
            (int32_t)(frame.x + x),
            (int32_t)frame.y,
            1,
            (uint32_t)frame.height
        };
        nx_color_t c = {col.r, col.g, col.b, col.a};
        nxgfx_fill_rect(ctx, seg, c);
    }
    
    /* Draw thumb indicator */
    float thumb_x = frame.x + (current_hue / 360.0f) * width;
    nx_rect_t thumb = {
        (int32_t)(thumb_x - 3),
        (int32_t)(frame.y - 2),
        6,
        (uint32_t)(frame.height + 4)
    };
    nxgfx_fill_rounded_rect(ctx, thumb, NX_COLOR_WHITE, 3);
    
    /* Thumb border */
    nx_rect_t thumb_inner = {thumb.x + 1, thumb.y + 1, 4, thumb.height - 2};
    nx_color_t border = {0, 0, 0, 128};
    nxgfx_draw_rect(ctx, thumb_inner, border, 1);
}

void color_picker_view_render(rx_color_picker_view* view, void* ctx) {
    if (!view || !ctx || !view->base.visible) return;
    
    nx_context_t* gfx = (nx_context_t*)ctx;
    rx_rect frame = view->base.box.frame;
    float x = frame.x + view->margin;
    float y = frame.y + view->margin;
    
    /* Hue slider */
    if (view->show_hue_slider) {
        rx_rect slider_frame = {x, y, 400, 20};
        render_rainbow_slider(gfx, slider_frame, view->current_hsl.h);
        y += 32;
        
        /* Current hue palette */
        rx_color custom_palette[10];
        color_picker_generate_palette(view->current_hsl.h, custom_palette, 10);
        
        for (int i = 0; i < 10; i++) {
            nx_rect_t swatch = {
                (int32_t)(x + i * (view->swatch_size + view->swatch_gap)),
                (int32_t)y,
                (uint32_t)view->swatch_size,
                (uint32_t)view->swatch_size
            };
            nx_color_t c = {custom_palette[i].r, custom_palette[i].g, 
                           custom_palette[i].b, custom_palette[i].a};
            nxgfx_fill_rounded_rect(gfx, swatch, c, 6);
        }
        y += view->swatch_size + 24;
    }
    
    /* Predefined palettes */
    if (view->show_palettes) {
        for (size_t p = 0; p < view->palette_count; p++) {
            rx_color_palette* pal = &view->palettes[p];
            
            /* Palette label */
            y += 20;
            
            /* Color swatches */
            for (int i = 0; i < 10; i++) {
                nx_rect_t swatch = {
                    (int32_t)(x + i * (view->swatch_size + view->swatch_gap)),
                    (int32_t)y,
                    (uint32_t)view->swatch_size,
                    (uint32_t)view->swatch_size
                };
                nx_color_t c = {pal->colors[i].r, pal->colors[i].g,
                               pal->colors[i].b, pal->colors[i].a};
                nxgfx_fill_rounded_rect(gfx, swatch, c, 6);
                
                /* Selection indicator */
                if (view->selected_palette == (int)p && view->selected_swatch == i) {
                    nx_rect_t sel = {swatch.x - 2, swatch.y - 2, 
                                    swatch.width + 4, swatch.height + 4};
                    nx_color_t border = {0, 122, 255, 255};
                    nxgfx_draw_rect(gfx, sel, border, 2);
                }
            }
            y += view->swatch_size + 8;
        }
    }
    
    /* Preview panel on right */
    if (view->show_preview) {
        float preview_x = frame.x + frame.width - view->margin - 200;
        float preview_y = frame.y + view->margin;
        
        /* Preview rectangle */
        nx_rect_t preview = {
            (int32_t)preview_x, (int32_t)preview_y, 200, 150
        };
        nx_color_t prev_c = {view->selected_color.r, view->selected_color.g,
                            view->selected_color.b, view->selected_color.a};
        nxgfx_fill_rounded_rect(gfx, preview, prev_c, 12);
        
        if (view->show_values) {
            char hex_buf[16], rgb_buf[32];
            color_picker_format_hex(view->selected_color, hex_buf, sizeof(hex_buf));
            color_picker_format_rgb(view->selected_color, rgb_buf, sizeof(rgb_buf));
            
            /* Value display boxes */
            nx_rect_t hex_box = {(int32_t)preview_x, (int32_t)(preview_y + 160), 200, 40};
            nx_color_t bg = {40, 40, 44, 255};
            nxgfx_fill_rounded_rect(gfx, hex_box, bg, 8);
            
            nx_rect_t rgb_box = {(int32_t)preview_x, (int32_t)(preview_y + 210), 200, 40};
            nxgfx_fill_rounded_rect(gfx, rgb_box, bg, 8);
        }
    }
}

bool color_picker_view_handle_event(rx_color_picker_view* view, 
                                     rx_point pos, bool pressed) {
    if (!view || !view->base.enabled || !pressed) return false;
    
    rx_rect frame = view->base.box.frame;
    float x = frame.x + view->margin;
    float y = frame.y + view->margin;
    
    /* Check hue slider */
    if (view->show_hue_slider) {
        rx_rect slider = {x, y, 400, 20};
        if (pos.x >= slider.x && pos.x < slider.x + slider.width &&
            pos.y >= slider.y && pos.y < slider.y + slider.height) {
            float hue = (pos.x - slider.x) / slider.width * 360.0f;
            color_picker_set_hue(view, hue);
            return true;
        }
        y += 32;
        
        /* Check custom hue palette */
        for (int i = 0; i < 10; i++) {
            float sx = x + i * (view->swatch_size + view->swatch_gap);
            if (pos.x >= sx && pos.x < sx + view->swatch_size &&
                pos.y >= y && pos.y < y + view->swatch_size) {
                rx_color custom[10];
                color_picker_generate_palette(view->current_hsl.h, custom, 10);
                color_picker_set_color(view, custom[i]);
                view->selected_palette = -1;
                view->selected_swatch = i;
                return true;
            }
        }
        y += view->swatch_size + 24;
    }
    
    /* Check predefined palettes */
    if (view->show_palettes) {
        for (size_t p = 0; p < view->palette_count; p++) {
            y += 20;
            
            for (int i = 0; i < 10; i++) {
                float sx = x + i * (view->swatch_size + view->swatch_gap);
                if (pos.x >= sx && pos.x < sx + view->swatch_size &&
                    pos.y >= y && pos.y < y + view->swatch_size) {
                    color_picker_set_color(view, view->palettes[p].colors[i]);
                    view->selected_palette = (int)p;
                    view->selected_swatch = i;
                    return true;
                }
            }
            y += view->swatch_size + 8;
        }
    }
    
    return false;
}
