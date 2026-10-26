/*
 * REOX Runtime Library - Implementation
 * Standard library for REOX compiled programs
 *
 * OS independent: all memory, output, time and math go through
 * platform/rx_platform.h. The libc-style helpers live in rx_core.c.
 */

#include "rx_core.h"

/* ============================================================================
 * Print Functions
 * ============================================================================ */

void println(rx_string s) {
    rx_fdputs(RX_PLAT_STDOUT, s);
    rx_write_all(RX_PLAT_STDOUT, "\n", 1);
}

void print(rx_string s) {
    rx_fdputs(RX_PLAT_STDOUT, s);
}

void print_int(rx_int n) {
    rx_printf("%lld", (long long)n);
}

void print_float(rx_float n) {
    rx_printf("%g", n);
}

void print_bool(rx_bool b) {
    rx_fdputs(RX_PLAT_STDOUT, b ? "true" : "false");
}

void printf_rx(rx_string fmt, ...) {
    va_list args;
    va_start(args, fmt);
    rx_vfdprintf(RX_PLAT_STDOUT, fmt, args);
    va_end(args);
}

/* ============================================================================
 * String Object Functions
 * ============================================================================ */

static rx_str str_invalid(void) {
    rx_str r = { NULL, 0, 0 };
    return r;
}

/* Allocate an rx_str able to hold len characters plus the terminator. */
static rx_str str_alloc(size_t len) {
    rx_str result = str_invalid();
    if (len == SIZE_MAX) return result;
    result.data = (char*)rx_alloc(len + 1);
    if (!result.data) return result;
    result.len = len;
    result.capacity = len + 1;
    result.data[0] = '\0';
    return result;
}

rx_str str_new(const char* s) {
    if (RX_UNLIKELY(s == NULL)) return str_invalid();
    size_t len = rx_strlen(s);
    rx_str result = str_alloc(len);
    if (result.data) rx_memcpy(result.data, s, len + 1);
    return result;
}

rx_str str_with_capacity(size_t capacity) {
    rx_str result = str_invalid();
    if (capacity == 0) capacity = 16;
    result.data = (char*)rx_alloc(capacity);
    if (!result.data) return result;
    result.capacity = capacity;
    result.len = 0;
    result.data[0] = '\0';
    return result;
}

void str_free(rx_str* s) {
    if (s && s->data) {
        rx_free(s->data);
        s->data = NULL;
        s->len = 0;
        s->capacity = 0;
    }
}

size_t str_len(const rx_str* s) {
    return s ? s->len : 0;
}

rx_str str_concat(const rx_str* a, const rx_str* b) {
    size_t alen = (a && a->data) ? a->len : 0;
    size_t blen = (b && b->data) ? b->len : 0;
    if (alen > SIZE_MAX - blen - 1) return str_invalid();
    rx_str result = str_alloc(alen + blen);
    if (!result.data) return result;
    if (alen) rx_memcpy(result.data, a->data, alen);
    if (blen) rx_memcpy(result.data + alen, b->data, blen);
    result.data[alen + blen] = '\0';
    return result;
}

bool str_append(rx_str* s, const char* suffix) {
    if (RX_UNLIKELY(!s || !suffix)) return false;
    size_t suffix_len = rx_strlen(suffix);
    if (s->len > SIZE_MAX - suffix_len - 1) return false;
    size_t new_len = s->len + suffix_len;

    if (!s->data || new_len + 1 > s->capacity) {
        size_t new_capacity = new_len + 1;
        if (new_capacity <= SIZE_MAX / 2) new_capacity *= 2;
        char* new_data = (char*)rx_realloc(s->data, new_capacity);
        if (RX_UNLIKELY(!new_data)) return false;
        if (!s->data) { new_data[0] = '\0'; s->len = 0; }
        s->data = new_data;
        s->capacity = new_capacity;
    }

    rx_memcpy(s->data + s->len, suffix, suffix_len + 1);
    s->len = new_len;
    return true;
}

int str_cmp(const rx_str* a, const rx_str* b) {
    if (!a || !a->data) return (b && b->data) ? -1 : 0;
    if (!b || !b->data) return 1;
    return rx_strcmp(a->data, b->data);
}

bool str_eq(const rx_str* a, const rx_str* b) {
    return str_cmp(a, b) == 0;
}

rx_str str_substr(const rx_str* s, size_t start, size_t len) {
    if (RX_UNLIKELY(!s || !s->data || start > s->len)) return str_invalid();
    if (len > s->len - start) len = s->len - start;
    rx_str result = str_alloc(len);
    if (!result.data) return result;
    rx_memcpy(result.data, s->data + start, len);
    result.data[len] = '\0';
    return result;
}

int64_t str_find(const rx_str* s, const char* needle) {
    if (RX_UNLIKELY(!s || !s->data || !needle)) return -1;
    const char* found = rx_strstr(s->data, needle);
    return found ? (int64_t)(found - s->data) : -1;
}

bool str_contains(const rx_str* s, const char* needle) {
    return str_find(s, needle) >= 0;
}

bool str_starts_with(const rx_str* s, const char* prefix) {
    if (RX_UNLIKELY(!s || !s->data || !prefix)) return false;
    size_t prefix_len = rx_strlen(prefix);
    if (prefix_len > s->len) return false;
    return rx_memcmp(s->data, prefix, prefix_len) == 0;
}

bool str_ends_with(const rx_str* s, const char* suffix) {
    if (RX_UNLIKELY(!s || !s->data || !suffix)) return false;
    size_t suffix_len = rx_strlen(suffix);
    if (suffix_len > s->len) return false;
    return rx_memcmp(s->data + s->len - suffix_len, suffix, suffix_len) == 0;
}

static rx_str str_map_case(const rx_str* s, bool upper) {
    if (RX_UNLIKELY(!s || !s->data)) return str_invalid();
    rx_str result = str_alloc(s->len);
    if (!result.data) return result;
    for (size_t i = 0; i < s->len; i++) {
        int c = (unsigned char)s->data[i];
        result.data[i] = (char)(upper ? rx_toupper(c) : rx_tolower(c));
    }
    result.data[s->len] = '\0';
    return result;
}

rx_str str_to_upper(const rx_str* s) { return str_map_case(s, true); }
rx_str str_to_lower(const rx_str* s) { return str_map_case(s, false); }

rx_str str_trim(const rx_str* s) {
    if (RX_UNLIKELY(!s || !s->data)) return str_invalid();
    size_t start = 0;
    while (start < s->len && rx_isspace((unsigned char)s->data[start])) start++;
    size_t end = s->len;
    while (end > start && rx_isspace((unsigned char)s->data[end - 1])) end--;
    return str_substr(s, start, end - start);
}

rx_array str_split(const rx_str* s, char delimiter) {
    rx_array result = array_new(sizeof(rx_str), 8);
    if (RX_UNLIKELY(!s || !s->data || !result.data)) return result;

    size_t start = 0;
    for (size_t i = 0; i <= s->len; i++) {
        if (i == s->len || s->data[i] == delimiter) {
            rx_str part = str_substr(s, start, i - start);
            if (!part.data || !array_push(&result, &part)) {
                /* out of memory: release what we built and report an empty array */
                str_free(&part);
                for (size_t j = 0; j < result.len; j++) {
                    str_free((rx_str*)array_get(&result, j));
                }
                array_free(&result);
                return result;
            }
            start = i + 1;
        }
    }
    return result;
}

rx_str int_to_str(rx_int n) {
    char buffer[32];
    rx_snprintf(buffer, sizeof(buffer), "%lld", (long long)n);
    return str_new(buffer);
}

rx_str float_to_str(rx_float n) {
    char buffer[64];
    rx_snprintf(buffer, sizeof(buffer), "%g", n);
    return str_new(buffer);
}

rx_int str_to_int(const rx_str* s) {
    if (RX_UNLIKELY(!s || !s->data)) return 0;
    return (rx_int)rx_atoll(s->data);
}

rx_float str_to_float(const rx_str* s) {
    if (RX_UNLIKELY(!s || !s->data)) return 0.0;
    return rx_atof(s->data);
}

/* ============================================================================
 * Math Functions (platform shim)
 * ============================================================================ */

rx_float abs_float(rx_float x)                 { return rx_plat_fabs(x); }
rx_float pow_float(rx_float base, rx_float e)  { return rx_plat_pow(base, e); }
rx_float sqrt_float(rx_float x)                { return rx_plat_sqrt(x); }
rx_float cbrt_float(rx_float x)                { return rx_plat_cbrt(x); }
rx_float sin_float(rx_float x)                 { return rx_plat_sin(x); }
rx_float cos_float(rx_float x)                 { return rx_plat_cos(x); }
rx_float tan_float(rx_float x)                 { return rx_plat_tan(x); }
rx_float asin_float(rx_float x)                { return rx_plat_asin(x); }
rx_float acos_float(rx_float x)                { return rx_plat_acos(x); }
rx_float atan_float(rx_float x)                { return rx_plat_atan(x); }
rx_float atan2_float(rx_float y, rx_float x)   { return rx_plat_atan2(y, x); }
rx_float exp_float(rx_float x)                 { return rx_plat_exp(x); }
rx_float log_float(rx_float x)                 { return rx_plat_log(x); }
rx_float log10_float(rx_float x)               { return rx_plat_log10(x); }
rx_float log2_float(rx_float x)                { return rx_plat_log2(x); }
rx_float floor_float(rx_float x)               { return rx_plat_floor(x); }
rx_float ceil_float(rx_float x)                { return rx_plat_ceil(x); }
rx_float round_float(rx_float x)               { return rx_plat_round(x); }
rx_float trunc_float(rx_float x)               { return rx_plat_trunc(x); }

/* ============================================================================
 * Array Functions
 * ============================================================================ */

rx_array array_new(size_t elem_size, size_t initial_capacity) {
    rx_array arr = { NULL, 0, 0, elem_size };
    if (elem_size == 0) return arr;
    size_t cap = initial_capacity > 0 ? initial_capacity : 8;
    if (cap > SIZE_MAX / elem_size) return arr;
    arr.data = rx_alloc(cap * elem_size);
    if (arr.data) arr.capacity = cap;
    return arr;
}

void array_free(rx_array* arr) {
    if (arr && arr->data) {
        rx_free(arr->data);
        arr->data = NULL;
        arr->len = 0;
        arr->capacity = 0;
    }
}

size_t array_len(const rx_array* arr) {
    return arr ? arr->len : 0;
}

bool array_push(rx_array* arr, const void* elem) {
    if (RX_UNLIKELY(!arr || !elem || arr->elem_size == 0)) return false;

    if (!arr->data || arr->len >= arr->capacity) {
        size_t new_capacity = arr->capacity ? arr->capacity * 2 : 8;
        if (new_capacity < arr->capacity || new_capacity > SIZE_MAX / arr->elem_size) return false;
        void* new_data = rx_realloc(arr->data, new_capacity * arr->elem_size);
        if (RX_UNLIKELY(!new_data)) return false;
        arr->data = new_data;
        arr->capacity = new_capacity;
    }

    char* dest = (char*)arr->data + (arr->len * arr->elem_size);
    rx_memcpy(dest, elem, arr->elem_size);
    arr->len++;
    return true;
}

void array_pop(rx_array* arr, void* out_elem) {
    if (RX_UNLIKELY(!arr || !arr->data || arr->len == 0)) return;
    arr->len--;
    if (out_elem) {
        char* src = (char*)arr->data + (arr->len * arr->elem_size);
        rx_memcpy(out_elem, src, arr->elem_size);
    }
}

void* array_get(rx_array* arr, size_t index) {
    if (RX_UNLIKELY(!arr || !arr->data || index >= arr->len)) return NULL;
    return (char*)arr->data + (index * arr->elem_size);
}

void array_set(rx_array* arr, size_t index, const void* elem) {
    if (RX_UNLIKELY(!arr || !arr->data || !elem || index >= arr->len)) return;
    char* dest = (char*)arr->data + (index * arr->elem_size);
    rx_memcpy(dest, elem, arr->elem_size);
}

/* ============================================================================
 * Utility Functions
 * ============================================================================ */

RX_NORETURN void rx_exit(int code) {
    rx_plat_exit(code);
}

RX_NORETURN RX_COLD void rx_panic(rx_string message) {
    rx_eprintf("PANIC: %s\n", message ? message : "unknown error");
    rx_plat_exit(1);
}

void rx_assert(bool condition, rx_string message) {
    if (RX_UNLIKELY(!condition)) {
        rx_eprintf("ASSERTION FAILED: %s\n", message ? message : "assertion failed");
        rx_plat_exit(1);
    }
}

uint64_t rx_time_ms(void) {
    return rx_plat_time_ms();
}

void rx_sleep_ms(uint64_t ms) {
    (void)rx_plat_sleep_ms(ms);
}
