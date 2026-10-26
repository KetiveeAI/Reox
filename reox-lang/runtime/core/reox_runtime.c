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

void print_str(const rx_str* s) {
    rx_fdputs(RX_PLAT_STDOUT, s ? str_cstr(s) : NULL);
}

void println_str(const rx_str* s) {
    print_str(s);
    rx_write_all(RX_PLAT_STDOUT, "\n", 1);
}

void println_int(rx_int n) {
    rx_printf("%lld\n", (long long)n);
}

void println_float(rx_float n) {
    rx_printf("%g\n", n);
}

void println_bool(rx_bool b) {
    rx_fdputs(RX_PLAT_STDOUT, b ? "true\n" : "false\n");
}

/* ============================================================================
 * String Object Functions
 * ============================================================================ */

static void str_dealloc(void* p) {
    rx_str* s = (rx_str*)p;
    if (s->capacity > 0) rx_free(s->data);
    rx_free(s);
}

/* Allocate an rx_str able to hold len characters plus the terminator. */
static rx_str* str_alloc(size_t len) {
    if (len == SIZE_MAX) return NULL;
    rx_str* s = (rx_str*)rx_alloc(sizeof(rx_str));
    if (!s) return NULL;
    s->data = (char*)rx_alloc(len + 1);
    if (!s->data) {
        rx_free(s);
        return NULL;
    }
    rx_object_init(&s->header, RX_TYPE_STR, str_dealloc);
    s->len = len;
    s->capacity = len + 1;
    s->data[0] = '\0';
    return s;
}

rx_str* str_from_bytes(const char* bytes, size_t len) {
    if (RX_UNLIKELY(bytes == NULL && len > 0)) return NULL;
    rx_str* s = str_alloc(len);
    if (!s) return NULL;
    if (len) rx_memcpy(s->data, bytes, len);
    s->data[len] = '\0';
    return s;
}

rx_str* str_new(const char* s) {
    if (RX_UNLIKELY(s == NULL)) return str_from_bytes("", 0);
    return str_from_bytes(s, rx_strlen(s));
}

rx_str* str_with_capacity(size_t capacity) {
    if (capacity == 0) capacity = 16;
    rx_str* s = str_alloc(capacity - 1);
    if (!s) return NULL;
    s->len = 0;
    s->data[0] = '\0';
    return s;
}

const char* str_cstr(const rx_str* s) {
    return (s && s->data) ? s->data : "";
}

size_t str_len(const rx_str* s) {
    return s ? s->len : 0;
}

rx_str* str_concat(const rx_str* a, const rx_str* b) {
    size_t alen = (a && a->data) ? a->len : 0;
    size_t blen = (b && b->data) ? b->len : 0;
    if (alen > SIZE_MAX - blen - 1) return NULL;
    rx_str* result = str_alloc(alen + blen);
    if (!result) return NULL;
    if (alen) rx_memcpy(result->data, a->data, alen);
    if (blen) rx_memcpy(result->data + alen, b->data, blen);
    result->data[alen + blen] = '\0';
    return result;
}

static bool str_append_bytes(rx_str* s, const char* bytes, size_t suffix_len) {
    if (RX_UNLIKELY(!s || (!bytes && suffix_len))) return false;
    if (s->capacity == 0) return false;      /* literal: storage not owned */
    if (s->len > SIZE_MAX - suffix_len - 1) return false;
    size_t new_len = s->len + suffix_len;

    if (new_len + 1 > s->capacity) {
        size_t new_capacity = new_len + 1;
        if (new_capacity <= SIZE_MAX / 2) new_capacity *= 2;
        char* new_data = (char*)rx_realloc(s->data, new_capacity);
        if (RX_UNLIKELY(!new_data)) return false;
        s->data = new_data;
        s->capacity = new_capacity;
    }

    if (suffix_len) rx_memcpy(s->data + s->len, bytes, suffix_len);
    s->data[new_len] = '\0';
    s->len = new_len;
    return true;
}

bool str_append(rx_str* s, const char* suffix) {
    if (RX_UNLIKELY(!suffix)) return false;
    return str_append_bytes(s, suffix, rx_strlen(suffix));
}

bool str_append_str(rx_str* s, const rx_str* suffix) {
    if (RX_UNLIKELY(!suffix)) return false;
    return str_append_bytes(s, suffix->data, suffix->len);
}

int str_cmp(const rx_str* a, const rx_str* b) {
    if (!a || !a->data) return (b && b->data) ? -1 : 0;
    if (!b || !b->data) return 1;
    return rx_strcmp(a->data, b->data);
}

bool str_eq(const rx_str* a, const rx_str* b) {
    return str_cmp(a, b) == 0;
}

rx_str* str_substr(const rx_str* s, size_t start, size_t len) {
    if (RX_UNLIKELY(!s || !s->data || start > s->len)) return NULL;
    if (len > s->len - start) len = s->len - start;
    return str_from_bytes(s->data + start, len);
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

static rx_str* str_map_case(const rx_str* s, bool upper) {
    if (RX_UNLIKELY(!s || !s->data)) return NULL;
    rx_str* result = str_alloc(s->len);
    if (!result) return NULL;
    for (size_t i = 0; i < s->len; i++) {
        int c = (unsigned char)s->data[i];
        result->data[i] = (char)(upper ? rx_toupper(c) : rx_tolower(c));
    }
    result->data[s->len] = '\0';
    return result;
}

rx_str* str_to_upper(const rx_str* s) { return str_map_case(s, true); }
rx_str* str_to_lower(const rx_str* s) { return str_map_case(s, false); }

rx_str* str_trim(const rx_str* s) {
    if (RX_UNLIKELY(!s || !s->data)) return NULL;
    size_t start = 0;
    while (start < s->len && rx_isspace((unsigned char)s->data[start])) start++;
    size_t end = s->len;
    while (end > start && rx_isspace((unsigned char)s->data[end - 1])) end--;
    return str_substr(s, start, end - start);
}

rx_array* str_split(const rx_str* s, char delimiter) {
    rx_array* result = array_new(sizeof(rx_str*), 8, &rx_ref_elem_ops);
    if (RX_UNLIKELY(!result)) return NULL;
    if (RX_UNLIKELY(!s || !s->data)) return result;

    size_t start = 0;
    for (size_t i = 0; i <= s->len; i++) {
        if (i == s->len || s->data[i] == delimiter) {
            rx_str* part = str_substr(s, start, i - start);
            bool stored = part && array_push(result, &part);
            rx_release(part);                 /* the array holds its own reference */
            if (!stored) {
                rx_release(result);
                return NULL;
            }
            start = i + 1;
        }
    }
    return result;
}

rx_str* int_to_str(rx_int n) {
    char buffer[32];
    rx_snprintf(buffer, sizeof(buffer), "%lld", (long long)n);
    return str_new(buffer);
}

rx_str* float_to_str(rx_float n) {
    char buffer[64];
    rx_snprintf(buffer, sizeof(buffer), "%g", n);
    return str_new(buffer);
}

rx_str* bool_to_str(rx_bool b) {
    return str_new(b ? "true" : "false");
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
 * Element ownership hooks
 * ============================================================================ */

static void ref_slot_retain(void* slot)  { rx_retain(*(void**)slot); }
static void ref_slot_release(void* slot) { rx_clear((void**)slot); }

const rx_elem_ops rx_ref_elem_ops = { ref_slot_retain, ref_slot_release };

static void elems_release(const rx_elem_ops* ops, void* data, size_t elem_size, size_t count) {
    if (!ops || !ops->release || !data) return;
    for (size_t i = 0; i < count; i++) {
        ops->release((char*)data + i * elem_size);
    }
}

/* ============================================================================
 * Array Functions
 * ============================================================================ */

static void array_dealloc(void* p) {
    rx_array* arr = (rx_array*)p;
    elems_release(arr->ops, arr->data, arr->elem_size, arr->len);
    rx_free(arr->data);
    rx_free(arr);
}

rx_array* array_new(size_t elem_size, size_t initial_capacity, const rx_elem_ops* ops) {
    if (elem_size == 0) return NULL;
    size_t cap = initial_capacity > 0 ? initial_capacity : 8;
    if (cap > SIZE_MAX / elem_size) return NULL;
    rx_array* arr = (rx_array*)rx_alloc(sizeof(rx_array));
    if (!arr) return NULL;
    arr->data = rx_alloc(cap * elem_size);
    if (!arr->data) {
        rx_free(arr);
        return NULL;
    }
    rx_object_init(&arr->header, RX_TYPE_ARRAY, array_dealloc);
    arr->len = 0;
    arr->capacity = cap;
    arr->elem_size = elem_size;
    arr->ops = ops;
    return arr;
}

size_t array_len(const rx_array* arr) {
    return arr ? arr->len : 0;
}

static bool array_reserve_one(rx_array* arr) {
    if (arr->data && arr->len < arr->capacity) return true;
    size_t new_capacity = arr->capacity ? arr->capacity * 2 : 8;
    if (new_capacity < arr->capacity || new_capacity > SIZE_MAX / arr->elem_size) return false;
    void* new_data = rx_realloc(arr->data, new_capacity * arr->elem_size);
    if (RX_UNLIKELY(!new_data)) return false;
    arr->data = new_data;
    arr->capacity = new_capacity;
    return true;
}

bool array_push(rx_array* arr, const void* elem) {
    if (RX_UNLIKELY(!arr || !elem || arr->elem_size == 0)) return false;
    if (!array_reserve_one(arr)) return false;
    char* dest = (char*)arr->data + (arr->len * arr->elem_size);
    rx_memcpy(dest, elem, arr->elem_size);
    if (arr->ops && arr->ops->retain) arr->ops->retain(dest);
    arr->len++;
    return true;
}

bool array_pop(rx_array* arr, void* out_elem) {
    if (RX_UNLIKELY(!arr || !arr->data || arr->len == 0)) return false;
    arr->len--;
    char* src = (char*)arr->data + (arr->len * arr->elem_size);
    if (out_elem) {
        rx_memcpy(out_elem, src, arr->elem_size);
    } else if (arr->ops && arr->ops->release) {
        arr->ops->release(src);
    }
    return true;
}

void* array_get(rx_array* arr, size_t index) {
    if (RX_UNLIKELY(!arr || !arr->data || index >= arr->len)) return NULL;
    return (char*)arr->data + (index * arr->elem_size);
}

void* array_at(rx_array* arr, size_t index) {
    void* slot = array_get(arr, index);
    if (RX_UNLIKELY(!slot)) {
        rx_eprintf("array index %zu out of range (length %zu)\n", index, arr ? arr->len : 0);
        rx_plat_abort("array index out of range");
    }
    return slot;
}

void array_set(rx_array* arr, size_t index, const void* elem) {
    if (RX_UNLIKELY(!arr || !arr->data || !elem || index >= arr->len)) return;
    char* dest = (char*)arr->data + (index * arr->elem_size);
    if (arr->ops && arr->ops->retain) {
        /* Retain the incoming value before releasing the old one so that
         * storing an element into its own slot is safe. */
        char tmp[64];
        if (arr->elem_size <= sizeof tmp) {
            rx_memcpy(tmp, elem, arr->elem_size);
            arr->ops->retain(tmp);
            arr->ops->release(dest);
            rx_memcpy(dest, tmp, arr->elem_size);
            return;
        }
        arr->ops->retain((void*)elem);
        arr->ops->release(dest);
    }
    rx_memcpy(dest, elem, arr->elem_size);
}

void array_clear(rx_array* arr) {
    if (RX_UNLIKELY(!arr)) return;
    elems_release(arr->ops, arr->data, arr->elem_size, arr->len);
    arr->len = 0;
}

bool array_remove(rx_array* arr, size_t index) {
    if (RX_UNLIKELY(!arr || !arr->data || index >= arr->len)) return false;
    char* slot = (char*)arr->data + (index * arr->elem_size);
    if (arr->ops && arr->ops->release) arr->ops->release(slot);
    size_t tail = (arr->len - index - 1) * arr->elem_size;
    if (tail) rx_memmove(slot, slot + arr->elem_size, tail);
    arr->len--;
    return true;
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
