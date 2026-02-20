#ifndef SHIM_H
#define SHIM_H

#include <stdint.h>

// Define rx_color as macro to match generated Color struct 
#define rx_color Color

// Map reox_* functions to standard runtime functions
#define reox_vstack vstack
#define reox_hstack hstack
#define reox_spacer spacer
#define reox_grid_view grid_view

#define reox_text_view text_view
#define reox_button_view button_view
#define reox_toggle_view toggle_view

#define reox_view_set_background view_set_background
#define reox_view_set_padding view_set_padding
#define reox_view_set_corner_radius view_set_corner_radius
#define reox_view_set_shadow view_set_shadow
#define reox_view_add_child view_add_child
#define reox_view_set_size view_set_size

#define reox_text_view_set_text text_set_text
#define reox_text_set_font_size text_set_font_size
#define reox_text_set_font_weight text_set_font_weight
#define reox_text_set_color text_set_color
#define reox_text_set_align text_set_align

#define reox_button_set_style button_set_style
#define reox_button_set_on_click button_set_on_click

#define reox_app_new app_new
#define reox_app_create_window app_create_window
#define reox_app_run app_run
#define reox_window_set_root window_set_root

#define reox_insets_all insets_all
#define reox_color_rgba color_rgba
#define reox_color_surface color_surface
#define reox_color_background color_background
#define reox_color_text color_text
#define reox_color_primary color_primary
#define reox_color_secondary color_secondary
#define reox_color_accent color_accent

#endif
