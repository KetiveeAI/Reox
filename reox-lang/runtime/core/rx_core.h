/*
 * REOX Runtime - internal core header
 *
 * Included by every translation unit under runtime/core/. It pulls in the
 * public runtime API, the platform contract and a handful of inline helpers
 * (float math wrappers, infinity constants) that core modules share.
 *
 * Core code must never include a libc or OS header: only <stdint.h>,
 * <stddef.h>, <stdbool.h> and <stdarg.h> are permitted (enforced by
 * `make check`). Everything else comes from reox_runtime.h (the freestanding
 * libc subset implemented in rx_core.c) or from rx_platform.h.
 */

#ifndef RX_CORE_H
#define RX_CORE_H

#include "reox_runtime.h"
#include "../platform/rx_platform.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ============================================================================
 * Floating point constants (compiler builtins, no <math.h>)
 * ============================================================================ */

#define RX_INFINITY   (__builtin_inf())
#define RX_INFINITYF  (__builtin_inff())
#define RX_NAN        (__builtin_nan(""))

/* ============================================================================
 * Float wrappers over the double platform math shim
 * ============================================================================ */

static inline float rx_powf(float a, float b)   { return (float)rx_plat_pow((double)a, (double)b); }
static inline float rx_sqrtf(float x)           { return (float)rx_plat_sqrt((double)x); }
static inline float rx_sinf(float x)            { return (float)rx_plat_sin((double)x); }
static inline float rx_cosf(float x)            { return (float)rx_plat_cos((double)x); }
static inline float rx_tanf(float x)            { return (float)rx_plat_tan((double)x); }
static inline float rx_atan2f(float y, float x) { return (float)rx_plat_atan2((double)y, (double)x); }
static inline float rx_expf(float x)            { return (float)rx_plat_exp((double)x); }
static inline float rx_logf(float x)            { return (float)rx_plat_log((double)x); }
static inline float rx_fmodf(float x, float y)  { return (float)rx_plat_fmod((double)x, (double)y); }
static inline float rx_floorf(float x)          { return (float)rx_plat_floor((double)x); }
static inline float rx_ceilf(float x)           { return (float)rx_plat_ceil((double)x); }
static inline float rx_roundf(float x)          { return (float)rx_plat_round((double)x); }
static inline float rx_fabsf(float x)           { return x < 0.0f ? -x : x; }
static inline float rx_fminf(float a, float b)  { return a < b ? a : b; }
static inline float rx_fmaxf(float a, float b)  { return a > b ? a : b; }
static inline double rx_fmin(double a, double b) { return a < b ? a : b; }
static inline double rx_fmax(double a, double b) { return a > b ? a : b; }

#ifdef __cplusplus
}
#endif

#endif /* RX_CORE_H */
