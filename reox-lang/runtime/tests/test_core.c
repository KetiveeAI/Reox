/*
 * Host-side checks for the freestanding libc subset in core/rx_core.c.
 * Compares rx_snprintf/rx_atof/rx_atoll and the string helpers against
 * glibc. Linked against librx_linux.a. Exit code 0 means every check passed.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include "../core/reox_runtime.h"

static int failures = 0;

static void expect_str(const char* what, const char* got, const char* want) {
    if (strcmp(got, want) != 0) {
        printf("FAIL %s: got '%s' want '%s'\n", what, got, want);
        failures++;
    }
}

#define CHECK_FMT(fmt, ...) do { \
    char a[256], b[256]; \
    rx_snprintf(a, sizeof a, fmt, __VA_ARGS__); \
    snprintf(b, sizeof b, fmt, __VA_ARGS__); \
    expect_str(fmt, a, b); \
} while (0)

int main(void) {
    CHECK_FMT("%d|%5d|%-5d|%05d|%+d", 42, 42, 42, 42, 42);
    CHECK_FMT("%ld %lld %u %zu", -7L, 1234567890123LL, 4000000000u, (size_t)99);
    CHECK_FMT("%x %X %#x %08x", 0xbeef, 0xbeef, 0xbeef, 0xbeef);
    CHECK_FMT("%s|%10s|%-10s|%.3s", "abc", "abc", "abc", "abcdef");
    CHECK_FMT("%c%c%%", 'x', 'y');
    CHECK_FMT("%f %.2f %.0f %10.3f", 3.14159, 3.14159, 2.5, -1.5);
    CHECK_FMT("%g %g %g %g %g", 0.0, 100000.0, 1000000.0, 0.0001, 0.00001);
    CHECK_FMT("%g %g %.3g %.10g", 3.14159, 1e-7, 1234.5678, 1.0 / 3.0);
    CHECK_FMT("%e %.2e %E", 12345.678, 0.000123, -5.0);
    CHECK_FMT("%f %f %.3f", 1e17, 123456789.125, 1e300);
    CHECK_FMT("%.20f %.17g %.17g", 0.1, 0.1, 1e-320);
    CHECK_FMT("%.1f %.1f %.1f", 0.05, 0.25, 0.35);
    CHECK_FMT("%ldx%ld", (long)800, (long)600);
    CHECK_FMT("rgba(%ld,%ld,%ld,%ld)", (long)1, (long)2, (long)3, (long)255);
    { char a[16]; const char* volatile np = NULL; rx_snprintf(a, sizeof a, "%s", np); expect_str("null %s", a, "(null)"); }

    /* nan / inf */
    { char a[64]; rx_snprintf(a, sizeof a, "%f %f %g", NAN, INFINITY, -INFINITY);
      expect_str("nan/inf", a, "nan inf -inf"); }

    /* truncation semantics */
    { char a[8]; int n = rx_snprintf(a, sizeof a, "%s", "hello world");
      if (n != 11 || strcmp(a, "hello w") != 0) { printf("FAIL truncation n=%d a='%s'\n", n, a); failures++; } }
    { int n = rx_snprintf(NULL, 0, "%d", 12345); if (n != 5) { printf("FAIL null buf n=%d\n", n); failures++; } }

    /* parsing */
    const char* nums[] = { "0", "42", "-17", " 123abc", "+9", "9223372036854775807", "-9223372036854775808", "99999999999999999999" };
    for (size_t i = 0; i < sizeof nums / sizeof *nums; i++) {
        long long want = strtoll(nums[i], NULL, 10);
        if (rx_atoll(nums[i]) != want) { printf("FAIL atoll '%s'\n", nums[i]); failures++; }
    }
    const char* floats[] = { "0", "3.5", "-2.25", "1e3", "2.5E-2", "  .5", "1.", "inf", "-inf", "123456789.123456789", "1e-320", "1e400" };
    for (size_t i = 0; i < sizeof floats / sizeof *floats; i++) {
        double want = strtod(floats[i], NULL), got = rx_atof(floats[i]);
        double tol = fabs(want) * 1e-15;
        if (!(fabs(got - want) <= tol || (isinf(got) && isinf(want) && (got > 0) == (want > 0)))) {
            printf("FAIL atof '%s' got %.17g want %.17g\n", floats[i], got, want); failures++;
        }
    }

    /* strings */
    if (rx_strlen("abc") != 3 || rx_strlen(NULL) != 0) { puts("FAIL strlen"); failures++; }
    if (rx_strcmp("a", "b") >= 0 || rx_strcmp("b", "a") <= 0 || rx_strcmp("x", "x") != 0 || rx_strcmp(NULL, "a") >= 0) { puts("FAIL strcmp"); failures++; }
    { char buf[4]; size_t n = rx_strlcpy(buf, "hello", sizeof buf);
      if (n != 5 || strcmp(buf, "hel") != 0) { puts("FAIL strlcpy"); failures++; } }
    { char buf[8] = "ab"; rx_strlcat(buf, "cdefgh", sizeof buf);
      if (strcmp(buf, "abcdefg") != 0) { printf("FAIL strlcat '%s'\n", buf); failures++; } }
    if (rx_strstr("hello world", "o w") != strstr("hello world", "o w")) { puts("FAIL strstr"); failures++; }
    if (rx_strrchr("a/b/c", '/') != strrchr("a/b/c", '/')) { puts("FAIL strrchr"); failures++; }

    /* rx_str */
    rx_str* s = str_new("Hello");
    if (!str_append(s, ", World") || s->len != 12) { puts("FAIL append"); failures++; }
    rx_str* up = str_to_upper(s);
    expect_str("to_upper", up->data, "HELLO, WORLD");
    rx_str* sub = str_substr(s, 7, 100);
    expect_str("substr", sub->data, "World");
    rx_str* t = str_new("  pad  ");
    rx_str* trimmed = str_trim(t);
    expect_str("trim", trimmed->data, "pad");
    rx_array* parts = str_split(s, ',');
    if (array_len(parts) != 2) { puts("FAIL split"); failures++; }
    if (rx_refcount(*(rx_str**)array_get(parts, 0)) != 1) { puts("FAIL split refcount"); failures++; }
    rx_release(parts);
    rx_release(s); rx_release(up); rx_release(sub); rx_release(t); rx_release(trimmed);
    rx_str* f = float_to_str(2.5);
    expect_str("float_to_str", f->data, "2.5");
    rx_release(f);
    { static rx_str lit = RX_STR_LIT_INIT("lit\n");
      if (lit.len != 4 || str_append(&lit, "x") || rx_refcount(&lit) != RX_REFCOUNT_IMMORTAL) { puts("FAIL literal"); failures++; }
      rx_retain(&lit); rx_release(&lit);
      if (rx_refcount(&lit) != RX_REFCOUNT_IMMORTAL) { puts("FAIL immortal"); failures++; } }

    /* sort */
    int v[] = { 5, 3, 9, 1, 7, 2, 8, 6, 4, 0 };
    int cmp(const void* a, const void* b);
    rx_sort(v, 10, sizeof(int), cmp);
    for (int i = 0; i < 10; i++) if (v[i] != i) { puts("FAIL sort"); failures++; break; }

    /* array growth */
    rx_array* arr = array_new(sizeof(int), 1, NULL);
    for (int i = 0; i < 1000; i++) if (!array_push(arr, &i)) { puts("FAIL push"); failures++; break; }
    if (array_len(arr) != 1000 || *(int*)array_get(arr, 999) != 999) { puts("FAIL array"); failures++; }
    if (!array_remove(arr, 0) || *(int*)array_at(arr, 0) != 1 || array_len(arr) != 999) { puts("FAIL remove"); failures++; }
    rx_release(arr);

    /* arrays of references retain their elements */
    rx_str* elem = str_new("elem");
    rx_array* refs = array_new(sizeof(rx_str*), 2, &rx_ref_elem_ops);
    array_push(refs, &elem); array_push(refs, &elem);
    if (rx_refcount(elem) != 3) { puts("FAIL ref push"); failures++; }
    rx_str* other = str_new("other");
    array_set(refs, 0, &other);
    if (rx_refcount(elem) != 2 || rx_refcount(other) != 2) { puts("FAIL ref set"); failures++; }
    rx_str* popped = NULL;
    array_pop(refs, &popped);
    if (popped != elem || rx_refcount(elem) != 2) { puts("FAIL ref pop"); failures++; }
    rx_release(popped);
    rx_release(refs);
    if (rx_refcount(elem) != 1 || rx_refcount(other) != 1) { puts("FAIL ref release"); failures++; }
    rx_release(elem); rx_release(other);

    /* map */
    rx_map* m = map_new(sizeof(int64_t), NULL);
    rx_str* ka = str_new("a"); rx_str* kb = str_new("b");
    int64_t one = 1, two = 2, three = 3;
    map_set(m, ka, &one); map_set(m, kb, &two); map_set(m, ka, &three);
    if (map_len(m) != 2 || *(int64_t*)map_get(m, ka) != 3 || !map_has(m, kb)) { puts("FAIL map"); failures++; }
    rx_array* keys = map_keys(m);
    if (array_len(keys) != 2 || !str_eq(*(rx_str**)array_get(keys, 1), kb)) { puts("FAIL map keys"); failures++; }
    rx_release(keys);
    if (!map_remove(m, ka) || map_has(m, ka) || map_len(m) != 1) { puts("FAIL map remove"); failures++; }
    rx_release(m);
    if (rx_refcount(ka) != 1 || rx_refcount(kb) != 1) { puts("FAIL map key refcount"); failures++; }
    rx_release(ka); rx_release(kb);

    printf("%s (%d failures)\n", failures ? "test_core: FAILED" : "test_core: ok", failures);
    return failures ? 1 : 0;
}

int cmp(const void* a, const void* b) { return *(const int*)a - *(const int*)b; }
