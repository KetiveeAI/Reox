/*
 * REOX Transition Runtime - Stdlib Wrappers
 * Maps transition.rx stdlib declarations to reox_animation.h implementation
 * 
 * Copyright (c) 2025-2026 KetiveeAI
 */

#ifndef REOX_TRANSITION_STDLIB_H
#define REOX_TRANSITION_STDLIB_H

#include "reox_animation.h"
#include "reox_ui.h"
#include <stdlib.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ============================================================================
 * Transition Type (matches transition.rx struct)
 * ============================================================================ */

typedef struct rx_transition {
    int64_t id;
    int32_t duration_ms;
    int32_t easing;  /* Maps to rx_easing */
} rx_transition;

/* ============================================================================
 * Transition Constructors
 * Maps: transition.rx extern fn declarations
 * ============================================================================ */

/* transition_linear(duration_ms: int) -> Transition */
static inline rx_transition transition_linear(int64_t duration_ms) {
    static int64_t next_id = 1;
    return (rx_transition){ next_id++, (int32_t)duration_ms, RX_EASE_LINEAR };
}

/* transition_ease(duration_ms: int) -> Transition */
static inline rx_transition transition_ease(int64_t duration_ms) {
    static int64_t next_id = 1;
    return (rx_transition){ next_id++, (int32_t)duration_ms, RX_EASE_IN_OUT };
}

/* transition_ease_in(duration_ms: int) -> Transition */
static inline rx_transition transition_ease_in(int64_t duration_ms) {
    static int64_t next_id = 1;
    return (rx_transition){ next_id++, (int32_t)duration_ms, RX_EASE_IN };
}

/* transition_ease_out(duration_ms: int) -> Transition */
static inline rx_transition transition_ease_out(int64_t duration_ms) {
    static int64_t next_id = 1;
    return (rx_transition){ next_id++, (int32_t)duration_ms, RX_EASE_OUT };
}

/* transition_ease_in_out(duration_ms: int) -> Transition */
static inline rx_transition transition_ease_in_out(int64_t duration_ms) {
    static int64_t next_id = 1;
    return (rx_transition){ next_id++, (int32_t)duration_ms, RX_EASE_IN_OUT };
}

/* transition_bounce(duration_ms: int) -> Transition */
static inline rx_transition transition_bounce(int64_t duration_ms) {
    static int64_t next_id = 1;
    return (rx_transition){ next_id++, (int32_t)duration_ms, RX_EASE_BOUNCE };
}

/* transition_elastic(duration_ms: int) -> Transition */
static inline rx_transition transition_elastic(int64_t duration_ms) {
    static int64_t next_id = 1;
    return (rx_transition){ next_id++, (int32_t)duration_ms, RX_EASE_OUT_ELASTIC };
}

/* transition_spring(stiffness: float, damping: float) -> Transition */
static inline rx_transition transition_spring(double stiffness, double damping) {
    static int64_t next_id = 1;
    /* Springs don't use duration - use 500ms as default for compatibility */
    (void)stiffness; (void)damping;
    return (rx_transition){ next_id++, 500, RX_EASE_SPRING };
}

/* transition_cubic_bezier(x1, y1, x2, y2, duration_ms) -> Transition */
static inline rx_transition transition_cubic_bezier(
    double x1, double y1, double x2, double y2, int64_t duration_ms) {
    static int64_t next_id = 1;
    (void)x1; (void)y1; (void)x2; (void)y2;
    /* Custom bezier stored separately - use ease-in-out as placeholder */
    return (rx_transition){ next_id++, (int32_t)duration_ms, RX_EASE_IN_OUT };
}

/* ============================================================================
 * View Animation Wrappers
 * Maps: transition.rx view_animate_* functions
 * ============================================================================ */

/* view_animate(v: View, property: string, to: float, t: Transition) -> void */
static inline void view_animate(rx_view* v, const char* property, double to, rx_transition t) {
    if (!v) return;
    
    rx_animatable_property prop = RX_PROP_X;
    if (strcmp(property, "x") == 0) prop = RX_PROP_X;
    else if (strcmp(property, "y") == 0) prop = RX_PROP_Y;
    else if (strcmp(property, "width") == 0) prop = RX_PROP_WIDTH;
    else if (strcmp(property, "height") == 0) prop = RX_PROP_HEIGHT;
    else if (strcmp(property, "opacity") == 0) prop = RX_PROP_OPACITY;
    else if (strcmp(property, "scale") == 0) prop = RX_PROP_SCALE;
    else if (strcmp(property, "rotation") == 0) prop = RX_PROP_ROTATION;
    
    rx_animation* anim = rx_view_animate(v, prop, (float)to, (float)t.duration_ms / 1000.0f);
    rx_anim_easing(anim, (rx_easing)t.easing);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* view_animate_opacity(v: View, to: float, t: Transition) -> void */
static inline void view_animate_opacity(rx_view* v, double to, rx_transition t) {
    if (!v) return;
    rx_animation* anim = rx_view_animate(v, RX_PROP_OPACITY, (float)to, (float)t.duration_ms / 1000.0f);
    rx_anim_easing(anim, (rx_easing)t.easing);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* view_fade_in(v: View, t: Transition) -> void */
static inline void view_fade_in(rx_view* v, rx_transition t) {
    if (!v) return;
    rx_animation* anim = rx_view_fade_in(v, (float)t.duration_ms / 1000.0f);
    rx_anim_easing(anim, (rx_easing)t.easing);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* view_fade_out(v: View, t: Transition) -> void */
static inline void view_fade_out(rx_view* v, rx_transition t) {
    if (!v) return;
    rx_animation* anim = rx_view_fade_out(v, (float)t.duration_ms / 1000.0f);
    rx_anim_easing(anim, (rx_easing)t.easing);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* view_animate_scale(v: View, to: float, t: Transition) -> void */
static inline void view_animate_scale(rx_view* v, double to, rx_transition t) {
    if (!v) return;
    rx_animation* anim = rx_view_animate(v, RX_PROP_SCALE, (float)to, (float)t.duration_ms / 1000.0f);
    rx_anim_easing(anim, (rx_easing)t.easing);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* view_scale_in(v: View, t: Transition) -> void */
static inline void view_scale_in(rx_view* v, rx_transition t) {
    if (!v) return;
    rx_animation* anim = rx_view_scale_in(v, (float)t.duration_ms / 1000.0f);
    rx_anim_easing(anim, (rx_easing)t.easing);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* view_scale_out(v: View, t: Transition) -> void */
static inline void view_scale_out(rx_view* v, rx_transition t) {
    if (!v) return;
    rx_animation* anim = rx_view_scale_out(v, (float)t.duration_ms / 1000.0f);
    rx_anim_easing(anim, (rx_easing)t.easing);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* view_animate_position(v: View, x: float, y: float, t: Transition) -> void */
static inline void view_animate_position(rx_view* v, double x, double y, rx_transition t) {
    if (!v) return;
    float duration = (float)t.duration_ms / 1000.0f;
    
    rx_animation* anim_x = rx_view_animate(v, RX_PROP_X, (float)x, duration);
    rx_anim_easing(anim_x, (rx_easing)t.easing);
    rx_anim_start(anim_x);
    rx_animator_add(rx_animator_shared(), anim_x);
    
    rx_animation* anim_y = rx_view_animate(v, RX_PROP_Y, (float)y, duration);
    rx_anim_easing(anim_y, (rx_easing)t.easing);
    rx_anim_start(anim_y);
    rx_animator_add(rx_animator_shared(), anim_y);
}

/* view_slide_in(v: View, direction: int, t: Transition) -> void */
static inline void view_slide_in(rx_view* v, int64_t direction, rx_transition t) {
    if (!v) return;
    float duration = (float)t.duration_ms / 1000.0f;
    float offset = 100.0f;  /* Slide distance */
    
    float from_x = 0, from_y = 0;
    switch (direction) {
        case 0: from_x = -offset; break;  /* left */
        case 1: from_x = offset; break;   /* right */
        case 2: from_y = -offset; break;  /* top */
        case 3: from_y = offset; break;   /* bottom */
    }
    
    rx_animation* anim = rx_view_slide_in(v, from_x, from_y, duration);
    rx_anim_easing(anim, (rx_easing)t.easing);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* view_slide_out(v: View, direction: int, t: Transition) -> void */
static inline void view_slide_out(rx_view* v, int64_t direction, rx_transition t) {
    if (!v) return;
    float duration = (float)t.duration_ms / 1000.0f;
    float offset = 100.0f;
    
    float to_x = 0, to_y = 0;
    switch (direction) {
        case 0: to_x = -offset; break;
        case 1: to_x = offset; break;
        case 2: to_y = -offset; break;
        case 3: to_y = offset; break;
    }
    
    rx_animation* anim = rx_view_slide_out(v, to_x, to_y, duration);
    rx_anim_easing(anim, (rx_easing)t.easing);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* view_animate_rotation(v: View, degrees: float, t: Transition) -> void */
static inline void view_animate_rotation(rx_view* v, double degrees, rx_transition t) {
    if (!v) return;
    rx_animation* anim = rx_view_animate(v, RX_PROP_ROTATION, (float)degrees, (float)t.duration_ms / 1000.0f);
    rx_anim_easing(anim, (rx_easing)t.easing);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* ============================================================================
 * Preset Animations
 * ============================================================================ */

/* preset_pulse(v: View, duration_ms: int) -> void */
static inline void preset_pulse(rx_view* v, int64_t duration_ms) {
    if (!v) return;
    rx_animation* anim = rx_view_pulse(v, (float)duration_ms / 1000.0f);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* preset_shake(v: View, intensity: float) -> void */
static inline void preset_shake(rx_view* v, double intensity) {
    if (!v) return;
    (void)intensity;
    rx_animation* anim = rx_view_shake(v, 0.5f);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* preset_bounce(v: View) -> void */
static inline void preset_bounce(rx_view* v) {
    if (!v) return;
    rx_animation* anim = rx_view_bounce(v, 0.5f);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* ============================================================================
 * Enter/Exit Transitions
 * ============================================================================ */

/* enter_fade(v: View, duration_ms: int) -> void */
static inline void enter_fade(rx_view* v, int64_t duration_ms) {
    if (!v) return;
    rx_animation* anim = rx_view_fade_in(v, (float)duration_ms / 1000.0f);
    rx_anim_easing(anim, RX_EASE_OUT);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* enter_slide_left(v: View, duration_ms: int) -> void */
static inline void enter_slide_left(rx_view* v, int64_t duration_ms) {
    if (!v) return;
    rx_animation* anim = rx_view_slide_in(v, -100.0f, 0.0f, (float)duration_ms / 1000.0f);
    rx_anim_easing(anim, RX_EASE_OUT);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* enter_slide_right(v: View, duration_ms: int) -> void */
static inline void enter_slide_right(rx_view* v, int64_t duration_ms) {
    if (!v) return;
    rx_animation* anim = rx_view_slide_in(v, 100.0f, 0.0f, (float)duration_ms / 1000.0f);
    rx_anim_easing(anim, RX_EASE_OUT);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* enter_zoom(v: View, duration_ms: int) -> void */
static inline void enter_zoom(rx_view* v, int64_t duration_ms) {
    if (!v) return;
    rx_animation* anim = rx_view_scale_in(v, (float)duration_ms / 1000.0f);
    rx_anim_easing(anim, RX_EASE_OUT_BACK);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* exit_fade(v: View, duration_ms: int) -> void */
static inline void exit_fade(rx_view* v, int64_t duration_ms) {
    if (!v) return;
    rx_animation* anim = rx_view_fade_out(v, (float)duration_ms / 1000.0f);
    rx_anim_easing(anim, RX_EASE_IN);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* exit_zoom(v: View, duration_ms: int) -> void */
static inline void exit_zoom(rx_view* v, int64_t duration_ms) {
    if (!v) return;
    rx_animation* anim = rx_view_scale_out(v, (float)duration_ms / 1000.0f);
    rx_anim_easing(anim, RX_EASE_IN);
    rx_anim_start(anim);
    rx_animator_add(rx_animator_shared(), anim);
}

/* ============================================================================
 * Keyframe Animation Wrappers
 * ============================================================================ */

typedef rx_keyframe_animation rx_keyframe_anim;

/* keyframe_animation() -> Animation */
static inline rx_keyframe_anim* keyframe_animation(void) {
    return rx_keyframes_create(1.0f);  /* Default 1s duration */
}

/* animation_add_keyframe(a: Animation, time: float, property: string, value: float) -> void */
static inline void animation_add_keyframe(rx_keyframe_anim* a, double time, 
                                          const char* property, double value) {
    if (!a) return;
    (void)property;  /* Property tracked separately */
    rx_keyframes_add(a, (float)time, (float)value, RX_EASE_LINEAR);
}

/* animation_set_duration(a: Animation, ms: int) -> void */
static inline void animation_set_duration(rx_keyframe_anim* a, int64_t ms) {
    if (!a) return;
    a->base.duration = (float)ms / 1000.0f;
}

/* animation_set_loop(a: Animation, count: int) -> void */
static inline void animation_set_loop(rx_keyframe_anim* a, int64_t count) {
    if (!a) return;
    a->base.repeat_count = (int)count;
}

/* animation_play(a: Animation, v: View) -> void */
static inline void animation_play(rx_keyframe_anim* a, rx_view* v) {
    if (!a || !v) return;
    (void)v;
    rx_anim_start(&a->base);
    rx_animator_add(rx_animator_shared(), &a->base);
}

/* animation_pause(a: Animation) -> void */
static inline void animation_pause(rx_keyframe_anim* a) {
    if (!a) return;
    rx_anim_pause(&a->base);
}

/* animation_stop(a: Animation) -> void */
static inline void animation_stop(rx_keyframe_anim* a) {
    if (!a) return;
    rx_anim_stop(&a->base);
}

#ifdef __cplusplus
}
#endif

#endif /* REOX_TRANSITION_STDLIB_H */
