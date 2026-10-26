/*
 * REOX Runtime Library - Header
 * Standard library for REOX compiled programs
 *
 * This header is OS independent. It declares the runtime's public API plus
 * the freestanding libc subset (rx_strlen, rx_snprintf, ...) that the runtime
 * uses instead of <string.h>/<stdio.h>, so that the same code builds against
 * glibc on the development host and against the NeolyxOS syscall layer.
 */

#ifndef REOX_RUNTIME_H
#define REOX_RUNTIME_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdarg.h>

#include "rx_object.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ============================================================================
 * Compiler Hints
 * ============================================================================ */

#ifdef __GNUC__
#define RX_INLINE       static inline __attribute__((always_inline))
#define RX_NOINLINE     __attribute__((noinline))
#define RX_PURE         __attribute__((pure))
#define RX_CONST        __attribute__((const))
#define RX_HOT          __attribute__((hot))
#define RX_COLD         __attribute__((cold))
#define RX_LIKELY(x)    __builtin_expect(!!(x), 1)
#define RX_UNLIKELY(x)  __builtin_expect(!!(x), 0)
#define RX_NORETURN     __attribute__((noreturn))
#define RX_FLATTEN      __attribute__((flatten))
#define RX_PRINTF(f, a) __attribute__((format(printf, f, a)))
#else
#define RX_INLINE       static inline
#define RX_NOINLINE
#define RX_PURE
#define RX_CONST
#define RX_HOT
#define RX_COLD
#define RX_LIKELY(x)    (x)
#define RX_UNLIKELY(x)  (x)
#define RX_NORETURN
#define RX_FLATTEN
#define RX_PRINTF(f, a)
#endif

/* ============================================================================
 * Type Definitions
 * ============================================================================ */

typedef int64_t rx_int;
typedef double rx_float;
typedef const char* rx_string;
typedef bool rx_bool;

/* ----------------------------------------------------------------------------
 * Reference counted heap objects
 *
 * rx_str, rx_array and rx_map all begin with an rx_object header and are
 * handled through pointers. A function documented as returning "+1" hands
 * the caller one strong reference that it must eventually rx_release; a
 * pointer parameter is borrowed for the duration of the call unless stated
 * otherwise. NULL means "allocation failed" wherever an object is returned.
 * -------------------------------------------------------------------------- */

/* Immutable or growable byte string, always NUL terminated.
 * capacity == 0 marks storage the object does not own (string literals). */
typedef struct rx_str {
    rx_object header;
    char* data;
    size_t len;
    size_t capacity;
} rx_str;

/* Initialiser for a static, immortal string literal object:
 *   static rx_str lit = RX_STR_LIT_INIT("text"); */
#define RX_STR_LIT_INIT(text) \
    { { RX_REFCOUNT_IMMORTAL, RX_TYPE_STR, NULL }, (char*)(text), sizeof(text) - 1, 0 }

/* How a container retains and releases the values stored in its slots.
 * NULL ops mean the elements are plain data (ints, floats, value structs
 * without references). */
typedef struct rx_elem_ops {
    void (*retain)(void* slot);
    void (*release)(void* slot);
} rx_elem_ops;

/* Ops for slots that hold one strong object pointer (rx_str*, rx_array*,
 * ref struct instances, ...). */
extern const rx_elem_ops rx_ref_elem_ops;

/* Growable array of fixed-size elements. */
typedef struct rx_array {
    rx_object header;
    void* data;
    size_t len;
    size_t capacity;
    size_t elem_size;
    const rx_elem_ops* ops;
} rx_array;

/* Insertion-ordered map from string keys to fixed-size values. Lookups are
 * linear in the number of entries; it is meant for small configuration-sized
 * maps, not for large tables. */
typedef struct rx_map {
    rx_object header;
    rx_str** keys;          /* strong references */
    void* values;
    size_t len;
    size_t capacity;
    size_t elem_size;
    const rx_elem_ops* ops;
} rx_map;

/* ============================================================================
 * Print Functions (standard output through the platform layer)
 * ============================================================================ */

extern void println(rx_string s);
extern void print(rx_string s);
extern void print_int(rx_int n);
extern void print_float(rx_float n);
extern void print_bool(rx_bool b);
extern void printf_rx(rx_string fmt, ...) RX_PRINTF(1, 2);

/* Object-aware printing used by compiled REOX code. NULL prints "(null)". */
extern void print_str(const rx_str* s);
extern void println_str(const rx_str* s);
extern void println_int(rx_int n);
extern void println_float(rx_float n);
extern void println_bool(rx_bool b);

/* ============================================================================
 * Inline Math Functions
 * ============================================================================ */

RX_INLINE RX_CONST rx_int abs_int(rx_int x) {
    return x < 0 ? -x : x;
}

RX_INLINE RX_CONST rx_int min_int(rx_int a, rx_int b) {
    return a < b ? a : b;
}

RX_INLINE RX_CONST rx_int max_int(rx_int a, rx_int b) {
    return a > b ? a : b;
}

RX_INLINE RX_CONST rx_float min_float(rx_float a, rx_float b) {
    return a < b ? a : b;
}

RX_INLINE RX_CONST rx_float max_float(rx_float a, rx_float b) {
    return a > b ? a : b;
}

RX_INLINE RX_CONST rx_int clamp_int(rx_int x, rx_int min_val, rx_int max_val) {
    return x < min_val ? min_val : (x > max_val ? max_val : x);
}

RX_INLINE RX_CONST rx_float clamp_float(rx_float x, rx_float min_val, rx_float max_val) {
    return x < min_val ? min_val : (x > max_val ? max_val : x);
}

/* ============================================================================
 * Math Functions (forwarded to the platform math shim)
 * ============================================================================ */

extern RX_PURE rx_float abs_float(rx_float x);
extern RX_PURE rx_float pow_float(rx_float base, rx_float exp);
extern RX_PURE rx_float sqrt_float(rx_float x);
extern RX_PURE rx_float cbrt_float(rx_float x);
extern RX_PURE rx_float sin_float(rx_float x);
extern RX_PURE rx_float cos_float(rx_float x);
extern RX_PURE rx_float tan_float(rx_float x);
extern RX_PURE rx_float asin_float(rx_float x);
extern RX_PURE rx_float acos_float(rx_float x);
extern RX_PURE rx_float atan_float(rx_float x);
extern RX_PURE rx_float atan2_float(rx_float y, rx_float x);
extern RX_PURE rx_float exp_float(rx_float x);
extern RX_PURE rx_float log_float(rx_float x);
extern RX_PURE rx_float log10_float(rx_float x);
extern RX_PURE rx_float log2_float(rx_float x);
extern RX_PURE rx_float floor_float(rx_float x);
extern RX_PURE rx_float ceil_float(rx_float x);
extern RX_PURE rx_float round_float(rx_float x);
extern RX_PURE rx_float trunc_float(rx_float x);

/* Constants */
#define RX_PI       3.14159265358979323846
#define RX_E        2.71828182845904523536
#define RX_TAU      6.28318530717958647692
#define RX_SQRT2    1.41421356237309504880

/* ============================================================================
 * String Object Functions
 * ============================================================================ */

/* Constructors return +1. */
extern rx_str* str_new(const char* s);
extern rx_str* str_from_bytes(const char* bytes, size_t len);
extern rx_str* str_with_capacity(size_t capacity);
/* Borrowed C view of the contents; "" for NULL. Valid while s is alive and
 * unmodified. */
extern RX_PURE const char* str_cstr(const rx_str* s);
extern RX_PURE size_t str_len(const rx_str* s);
extern rx_str* str_concat(const rx_str* a, const rx_str* b);             /* +1 */
/* In-place append. Returns false on out of memory or when s is a literal
 * (unowned storage); s is unchanged in both cases. */
extern bool str_append(rx_str* s, const char* suffix);
extern bool str_append_str(rx_str* s, const rx_str* suffix);
extern RX_PURE int str_cmp(const rx_str* a, const rx_str* b);
extern RX_PURE bool str_eq(const rx_str* a, const rx_str* b);
extern rx_str* str_substr(const rx_str* s, size_t start, size_t len);    /* +1 */
extern RX_PURE int64_t str_find(const rx_str* s, const char* needle);
extern RX_PURE bool str_contains(const rx_str* s, const char* needle);
extern RX_PURE bool str_starts_with(const rx_str* s, const char* prefix);
extern RX_PURE bool str_ends_with(const rx_str* s, const char* suffix);
extern rx_str* str_to_upper(const rx_str* s);                            /* +1 */
extern rx_str* str_to_lower(const rx_str* s);                            /* +1 */
extern rx_str* str_trim(const rx_str* s);                                /* +1 */
/* Array of rx_str* (ref elements), +1. */
extern rx_array* str_split(const rx_str* s, char delimiter);
extern rx_str* int_to_str(rx_int n);                                     /* +1 */
extern rx_str* float_to_str(rx_float n);                                 /* +1 */
extern rx_str* bool_to_str(rx_bool b);                                   /* +1 */
extern RX_PURE rx_int str_to_int(const rx_str* s);
extern RX_PURE rx_float str_to_float(const rx_str* s);

/* ============================================================================
 * Memory Functions
 * ============================================================================ */

/* All allocators return NULL on failure (and on size 0). */
extern void* rx_alloc(size_t size);
extern void* rx_calloc(size_t count, size_t size);
extern void* rx_realloc(void* ptr, size_t size);
extern void rx_free(void* ptr);
extern void rx_memcpy(void* dest, const void* src, size_t n);
extern void rx_memmove(void* dest, const void* src, size_t n);
extern void rx_memset(void* ptr, int value, size_t n);
extern RX_PURE int rx_memcmp(const void* a, const void* b, size_t n);

/* ============================================================================
 * Freestanding libc subset: strings and characters
 * ============================================================================ */

extern RX_PURE size_t rx_strlen(const char* s);                 /* NULL -> 0 */
extern RX_PURE int rx_strcmp(const char* a, const char* b);     /* NULL sorts first */
extern RX_PURE int rx_strncmp(const char* a, const char* b, size_t n);
/* Copy with truncation; always NUL terminates when cap > 0. Returns strlen(src). */
extern size_t rx_strlcpy(char* dst, const char* src, size_t cap);
/* Append with truncation. Returns the length it tried to create. */
extern size_t rx_strlcat(char* dst, const char* src, size_t cap);
extern RX_PURE const char* rx_strstr(const char* haystack, const char* needle);
extern RX_PURE const char* rx_strchr(const char* s, int c);
extern RX_PURE const char* rx_strrchr(const char* s, int c);
/* Heap copy via rx_alloc. NULL in -> NULL out; NULL on allocation failure. */
extern char* rx_strdup(const char* s);
/* Heap copy of at most n characters. */
extern char* rx_strndup(const char* s, size_t n);

extern RX_CONST bool rx_isspace(int c);
extern RX_CONST bool rx_isdigit(int c);
extern RX_CONST bool rx_isalpha(int c);
extern RX_CONST bool rx_isalnum(int c);
extern RX_CONST int rx_toupper(int c);
extern RX_CONST int rx_tolower(int c);

/* Number parsing. Leading whitespace skipped; stops at the first bad char. */
extern RX_PURE int64_t rx_atoll(const char* s);
extern RX_PURE double rx_atof(const char* s);

/* ============================================================================
 * Freestanding libc subset: formatting and output
 *
 * Supported conversions: %% %c %s %d %i %u %x %X %p %f %e %g with the
 * length modifiers l, ll, z, and the flags '-', '0', '+', ' ', width and
 * precision (including '*'). Floating point output is accurate to the last
 * printed digit for |x| < 1e18 and falls back to exponent form beyond that.
 * ============================================================================ */

extern int rx_vsnprintf(char* buf, size_t cap, const char* fmt, va_list ap);
extern int rx_snprintf(char* buf, size_t cap, const char* fmt, ...) RX_PRINTF(3, 4);
/* Formatted output to a platform descriptor (RX_PLAT_STDOUT = 1). */
extern int rx_vfdprintf(int fd, const char* fmt, va_list ap);
extern int rx_fdprintf(int fd, const char* fmt, ...) RX_PRINTF(2, 3);
extern int rx_printf(const char* fmt, ...) RX_PRINTF(1, 2);    /* stdout */
extern int rx_eprintf(const char* fmt, ...) RX_PRINTF(1, 2);   /* stderr */
/* Write every byte, retrying short writes. Returns bytes written or < 0. */
extern int64_t rx_write_all(int fd, const void* buf, size_t len);
/* Write a NUL terminated string (NULL -> "(null)"). */
extern int64_t rx_fdputs(int fd, const char* s);

/* ============================================================================
 * Freestanding libc subset: misc
 * ============================================================================ */

#define RX_RAND_MAX 0x7fffffff
/* Deterministic xorshift64* generator, range [0, RX_RAND_MAX]. */
extern int rx_rand(void);
extern void rx_srand(uint64_t seed);

/* In-place heapsort; no allocation, no recursion. */
extern void rx_sort(void* base, size_t count, size_t size,
                    int (*cmp)(const void*, const void*));

/* ============================================================================
 * Array Functions
 * ============================================================================ */

/* +1. ops may be NULL for plain data elements. */
extern rx_array* array_new(size_t elem_size, size_t initial_capacity, const rx_elem_ops* ops);
extern RX_PURE size_t array_len(const rx_array* arr);
/* Copy elem into the array, retaining it through ops. Returns false when the
 * element could not be stored (out of memory). */
extern bool array_push(rx_array* arr, const void* elem);
/* Move the last element into out_elem (ownership transfers, no retain) or
 * release it when out_elem is NULL. Returns false when the array is empty. */
extern bool array_pop(rx_array* arr, void* out_elem);
/* Borrowed pointer to the element, NULL when out of range. */
extern RX_HOT void* array_get(rx_array* arr, size_t index);
/* Borrowed pointer to the element; aborts the program when out of range. */
extern RX_HOT void* array_at(rx_array* arr, size_t index);
/* Replace an element: the new value is retained, the old one released. */
extern void array_set(rx_array* arr, size_t index, const void* elem);
/* Release every element and set len to 0. */
extern void array_clear(rx_array* arr);
/* Remove the element at index, releasing it. */
extern bool array_remove(rx_array* arr, size_t index);

RX_INLINE RX_PURE size_t array_len_fast(const rx_array* arr) {
    return arr ? arr->len : 0;
}

RX_INLINE RX_HOT void* array_get_unchecked(rx_array* arr, size_t index) {
    return (char*)arr->data + (index * arr->elem_size);
}

/* ============================================================================
 * Map Functions (string keys)
 * ============================================================================ */

/* +1. ops may be NULL for plain data values. */
extern rx_map* map_new(size_t elem_size, const rx_elem_ops* ops);
extern RX_PURE size_t map_len(const rx_map* m);
/* Insert or replace. The key is retained; the value is copied and retained
 * through ops. Returns false on out of memory. */
extern bool map_set(rx_map* m, rx_str* key, const void* value);
/* Borrowed pointer to the value, NULL when absent. */
extern void* map_get(rx_map* m, const rx_str* key);
/* Borrowed pointer to the value; aborts the program when the key is absent. */
extern void* rx_map_at(rx_map* m, const rx_str* key);
extern RX_PURE bool map_has(const rx_map* m, const rx_str* key);
/* Remove the entry, releasing key and value. Returns false when absent. */
extern bool map_remove(rx_map* m, const rx_str* key);
/* Release every entry. */
extern void map_clear(rx_map* m);
/* Array of rx_str* holding the keys in insertion order, +1. */
extern rx_array* map_keys(const rx_map* m);

/* ============================================================================
 * Utility Functions
 * ============================================================================ */

extern RX_NORETURN void rx_exit(int code);
extern RX_NORETURN RX_COLD void rx_panic(rx_string message);
extern void rx_assert(bool condition, rx_string message);

/* Monotonic milliseconds and sleep, through the platform layer. */
extern uint64_t rx_time_ms(void);
extern void rx_sleep_ms(uint64_t ms);

RX_INLINE void rx_assume(bool cond) {
#ifdef __GNUC__
    if (!cond) __builtin_unreachable();
#else
    (void)cond;
#endif
}

#ifdef __cplusplus
}
#endif

#endif /* REOX_RUNTIME_H */
