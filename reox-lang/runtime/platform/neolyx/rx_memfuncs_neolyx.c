/*
 * REOX Runtime - compiler support routines for the freestanding build
 *
 * GCC may emit calls to memcpy/memmove/memset/memcmp for struct copies and
 * initialisers even with -ffreestanding. NeolyxOS userspace normally gets
 * them from desktop/shell/userspace_stubs.c (memcpy, memset). These are weak
 * so the shell's strong definitions win when both are linked, and they live
 * in their own object so the archive member is only pulled in when needed.
 */

#include <stddef.h>
#include <stdint.h>

#if defined(__GNUC__)
#define RX_WEAK __attribute__((weak))
#define RX_NO_LOOP_PATTERNS __attribute__((optimize("no-tree-loop-distribute-patterns")))
#else
#define RX_WEAK
#define RX_NO_LOOP_PATTERNS
#endif

RX_WEAK RX_NO_LOOP_PATTERNS
void* memcpy(void* dest, const void* src, size_t n) {
    if (!dest || !src) return dest;
    unsigned char* d = (unsigned char*)dest;
    const unsigned char* s = (const unsigned char*)src;
    while (n--) *d++ = *s++;
    return dest;
}

RX_WEAK RX_NO_LOOP_PATTERNS
void* memmove(void* dest, const void* src, size_t n) {
    if (!dest || !src || dest == src) return dest;
    unsigned char* d = (unsigned char*)dest;
    const unsigned char* s = (const unsigned char*)src;
    if (d < s) {
        while (n--) *d++ = *s++;
    } else {
        d += n; s += n;
        while (n--) *--d = *--s;
    }
    return dest;
}

RX_WEAK RX_NO_LOOP_PATTERNS
void* memset(void* dest, int c, size_t n) {
    if (!dest) return dest;
    unsigned char* d = (unsigned char*)dest;
    while (n--) *d++ = (unsigned char)c;
    return dest;
}

RX_WEAK
int memcmp(const void* a, const void* b, size_t n) {
    if (a == b || n == 0) return 0;
    if (!a) return -1;
    if (!b) return 1;
    const unsigned char* x = (const unsigned char*)a;
    const unsigned char* y = (const unsigned char*)b;
    for (size_t i = 0; i < n; i++) {
        if (x[i] != y[i]) return (int)x[i] - (int)y[i];
    }
    return 0;
}
