/*
 * REOX Runtime - freestanding libc subset
 *
 * Implements the string, character, number parsing, formatting, PRNG and
 * sorting helpers declared in reox_runtime.h without any libc dependency.
 * Memory and output are routed through the platform contract.
 *
 * The byte loops below are deliberately simple. They carry a GCC attribute
 * that stops the loop-distribution pass from rewriting them into calls to
 * memcpy/memset, which would recurse on NeolyxOS where those symbols are
 * themselves implemented on top of these functions.
 */

#include "rx_core.h"

#if defined(__GNUC__)
#define RX_NO_LOOP_PATTERNS __attribute__((optimize("no-tree-loop-distribute-patterns")))
#else
#define RX_NO_LOOP_PATTERNS
#endif

/* ============================================================================
 * Memory
 * ============================================================================ */

RX_NO_LOOP_PATTERNS
void rx_memcpy(void* dest, const void* src, size_t n) {
    if (!dest || !src || n == 0) return;
    unsigned char* d = (unsigned char*)dest;
    const unsigned char* s = (const unsigned char*)src;
    while (n--) *d++ = *s++;
}

RX_NO_LOOP_PATTERNS
void rx_memmove(void* dest, const void* src, size_t n) {
    if (!dest || !src || n == 0 || dest == src) return;
    unsigned char* d = (unsigned char*)dest;
    const unsigned char* s = (const unsigned char*)src;
    if (d < s) {
        while (n--) *d++ = *s++;
    } else {
        d += n;
        s += n;
        while (n--) *--d = *--s;
    }
}

RX_NO_LOOP_PATTERNS
void rx_memset(void* ptr, int value, size_t n) {
    if (!ptr || n == 0) return;
    unsigned char* p = (unsigned char*)ptr;
    unsigned char v = (unsigned char)value;
    while (n--) *p++ = v;
}

int rx_memcmp(const void* a, const void* b, size_t n) {
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

void* rx_alloc(size_t size) {
    return rx_plat_alloc(size);
}

void* rx_calloc(size_t count, size_t size) {
    if (count == 0 || size == 0) return NULL;
    if (count > (SIZE_MAX / size)) return NULL;   /* overflow */
    size_t total = count * size;
    void* p = rx_plat_alloc(total);
    if (p) rx_memset(p, 0, total);
    return p;
}

void* rx_realloc(void* ptr, size_t size) {
    return rx_plat_realloc(ptr, size);
}

void rx_free(void* ptr) {
    rx_plat_free(ptr);
}

/* ============================================================================
 * Strings
 * ============================================================================ */

size_t rx_strlen(const char* s) {
    if (!s) return 0;
    size_t n = 0;
    while (s[n]) n++;
    return n;
}

int rx_strcmp(const char* a, const char* b) {
    if (a == b) return 0;
    if (!a) return -1;
    if (!b) return 1;
    while (*a && *a == *b) { a++; b++; }
    return (int)(unsigned char)*a - (int)(unsigned char)*b;
}

int rx_strncmp(const char* a, const char* b, size_t n) {
    if (n == 0 || a == b) return 0;
    if (!a) return -1;
    if (!b) return 1;
    while (n-- > 1 && *a && *a == *b) { a++; b++; }
    return (int)(unsigned char)*a - (int)(unsigned char)*b;
}

size_t rx_strlcpy(char* dst, const char* src, size_t cap) {
    size_t srclen = rx_strlen(src);
    if (!dst || cap == 0) return srclen;
    size_t n = srclen < cap - 1 ? srclen : cap - 1;
    if (src) rx_memcpy(dst, src, n);
    dst[n] = '\0';
    return srclen;
}

size_t rx_strlcat(char* dst, const char* src, size_t cap) {
    if (!dst || cap == 0) return rx_strlen(src);
    size_t dlen = 0;
    while (dlen < cap && dst[dlen]) dlen++;
    if (dlen == cap) return cap + rx_strlen(src);   /* dst not terminated */
    return dlen + rx_strlcpy(dst + dlen, src, cap - dlen);
}

const char* rx_strstr(const char* haystack, const char* needle) {
    if (!haystack || !needle) return NULL;
    if (!*needle) return haystack;
    size_t nlen = rx_strlen(needle);
    for (const char* p = haystack; *p; p++) {
        if (*p == *needle && rx_strncmp(p, needle, nlen) == 0) return p;
    }
    return NULL;
}

const char* rx_strchr(const char* s, int c) {
    if (!s) return NULL;
    char ch = (char)c;
    for (;; s++) {
        if (*s == ch) return s;
        if (!*s) return NULL;
    }
}

const char* rx_strrchr(const char* s, int c) {
    if (!s) return NULL;
    char ch = (char)c;
    const char* last = NULL;
    for (;; s++) {
        if (*s == ch) last = s;
        if (!*s) return last;
    }
}

char* rx_strdup(const char* s) {
    if (!s) return NULL;
    size_t len = rx_strlen(s);
    char* copy = (char*)rx_alloc(len + 1);
    if (!copy) return NULL;
    rx_memcpy(copy, s, len + 1);
    return copy;
}

char* rx_strndup(const char* s, size_t n) {
    if (!s) return NULL;
    size_t len = 0;
    while (len < n && s[len]) len++;
    char* copy = (char*)rx_alloc(len + 1);
    if (!copy) return NULL;
    rx_memcpy(copy, s, len);
    copy[len] = '\0';
    return copy;
}

/* ============================================================================
 * Characters (ASCII only; NeolyxOS has no locale)
 * ============================================================================ */

bool rx_isspace(int c) {
    return c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\v' || c == '\f';
}

bool rx_isdigit(int c) { return c >= '0' && c <= '9'; }

bool rx_isalpha(int c) {
    return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z');
}

bool rx_isalnum(int c) { return rx_isalpha(c) || rx_isdigit(c); }

int rx_toupper(int c) { return (c >= 'a' && c <= 'z') ? c - ('a' - 'A') : c; }

int rx_tolower(int c) { return (c >= 'A' && c <= 'Z') ? c + ('a' - 'A') : c; }

/* ============================================================================
 * Number parsing
 * ============================================================================ */

int64_t rx_atoll(const char* s) {
    if (!s) return 0;
    while (rx_isspace((unsigned char)*s)) s++;
    bool neg = false;
    if (*s == '-') { neg = true; s++; }
    else if (*s == '+') { s++; }
    uint64_t acc = 0;
    while (rx_isdigit((unsigned char)*s)) {
        unsigned d = (unsigned)(*s - '0');
        if (acc > (UINT64_MAX - d) / 10u) {           /* saturate */
            return neg ? INT64_MIN : INT64_MAX;
        }
        acc = acc * 10u + d;
        s++;
    }
    if (neg) {
        if (acc > (uint64_t)INT64_MAX + 1u) return INT64_MIN;
        return (int64_t)(0u - acc);
    }
    if (acc > (uint64_t)INT64_MAX) return INT64_MAX;
    return (int64_t)acc;
}

static double rx_pow10_int(int e) {
    double r = 1.0;
    double b = 10.0;
    bool neg = e < 0;
    if (neg) e = -e;
    while (e) {
        if (e & 1) r *= b;
        b *= b;
        e >>= 1;
    }
    return neg ? 1.0 / r : r;
}

double rx_atof(const char* s) {
    if (!s) return 0.0;
    while (rx_isspace((unsigned char)*s)) s++;
    bool neg = false;
    if (*s == '-') { neg = true; s++; }
    else if (*s == '+') { s++; }

    /* inf / nan */
    if ((s[0] == 'i' || s[0] == 'I') && (s[1] == 'n' || s[1] == 'N') && (s[2] == 'f' || s[2] == 'F')) {
        return neg ? -RX_INFINITY : RX_INFINITY;
    }
    if ((s[0] == 'n' || s[0] == 'N') && (s[1] == 'a' || s[1] == 'A') && (s[2] == 'n' || s[2] == 'N')) {
        return RX_NAN;
    }

    double mant = 0.0;
    int exp10 = 0;
    int digits = 0;
    while (rx_isdigit((unsigned char)*s)) {
        if (digits < 19) { mant = mant * 10.0 + (double)(*s - '0'); digits++; }
        else exp10++;                                   /* too many digits: scale */
        s++;
    }
    if (*s == '.') {
        s++;
        while (rx_isdigit((unsigned char)*s)) {
            if (digits < 19) { mant = mant * 10.0 + (double)(*s - '0'); digits++; exp10--; }
            s++;
        }
    }
    if (*s == 'e' || *s == 'E') {
        const char* save = s;
        s++;
        bool eneg = false;
        if (*s == '-') { eneg = true; s++; }
        else if (*s == '+') { s++; }
        if (rx_isdigit((unsigned char)*s)) {
            int e = 0;
            while (rx_isdigit((unsigned char)*s)) {
                if (e < 10000) e = e * 10 + (*s - '0');
                s++;
            }
            exp10 += eneg ? -e : e;
        } else {
            s = save;
        }
    }
    if (exp10 > 308) return neg ? -RX_INFINITY : RX_INFINITY;
    if (exp10 < -400) return neg ? -0.0 : 0.0;
    double v;
    if (exp10 < -300) v = (mant * rx_pow10_int(exp10 + 300)) * 1e-300;   /* avoid 1/inf */
    else v = mant * rx_pow10_int(exp10);
    return neg ? -v : v;
}

/* ============================================================================
 * Formatting
 * ============================================================================ */

static uint64_t rx_bits_of(double d) {
    union { double d; uint64_t u; } b;
    b.d = d;
    return b.u;
}

/* Output sink: either a bounded buffer or a descriptor with a small staging
 * buffer. total counts every byte that would have been written. */
typedef struct rx_sink {
    char* buf;
    size_t cap;
    size_t total;
    int fd;             /* -1 when writing to buf */
    char stage[256];
    size_t staged;
    int error;          /* set when a descriptor write failed */
} rx_sink;

static void rx_sink_flush(rx_sink* k) {
    if (k->fd < 0 || k->staged == 0) return;
    if (!k->error) {
        if (rx_write_all(k->fd, k->stage, k->staged) < 0) k->error = 1;
    }
    k->staged = 0;
}

static void rx_sink_putc(rx_sink* k, char c) {
    if (k->fd >= 0) {
        if (k->staged == sizeof(k->stage)) rx_sink_flush(k);
        k->stage[k->staged++] = c;
    } else if (k->buf && k->total + 1 < k->cap) {
        k->buf[k->total] = c;
    }
    k->total++;
}

static void rx_sink_puts(rx_sink* k, const char* s, size_t n) {
    for (size_t i = 0; i < n; i++) rx_sink_putc(k, s[i]);
}

static void rx_sink_pad(rx_sink* k, char c, size_t n) {
    while (n--) rx_sink_putc(k, c);
}

typedef struct rx_spec {
    bool left;      /* '-' */
    bool zero;      /* '0' */
    bool plus;      /* '+' */
    bool space;     /* ' ' */
    bool alt;       /* '#' */
    int width;
    int prec;       /* -1 when absent */
} rx_spec;

/* Emit a prepared string body with sign, width and zero-padding handling. */
static void rx_emit_number(rx_sink* k, const rx_spec* sp, const char* sign, size_t signlen,
                           const char* digits, size_t ndigits, size_t zero_prefix) {
    size_t body = signlen + zero_prefix + ndigits;
    size_t pad = (sp->width > 0 && (size_t)sp->width > body) ? (size_t)sp->width - body : 0;
    if (!sp->left && !sp->zero) rx_sink_pad(k, ' ', pad);
    rx_sink_puts(k, sign, signlen);
    if (!sp->left && sp->zero) rx_sink_pad(k, '0', pad);
    rx_sink_pad(k, '0', zero_prefix);
    rx_sink_puts(k, digits, ndigits);
    if (sp->left) rx_sink_pad(k, ' ', pad);
}

/* Unsigned integer to text in the given base. Returns number of chars. */
static size_t rx_utoa(uint64_t v, unsigned base, bool upper, char* out /* >= 24 */) {
    const char* digs = upper ? "0123456789ABCDEF" : "0123456789abcdef";
    char tmp[24];
    size_t n = 0;
    do {
        tmp[n++] = digs[v % base];
        v /= base;
    } while (v && n < sizeof(tmp));
    for (size_t i = 0; i < n; i++) out[i] = tmp[n - 1 - i];
    return n;
}

static void rx_fmt_integer(rx_sink* k, rx_spec* sp, bool is_signed, int64_t sv, uint64_t uv,
                           unsigned base, bool upper) {
    char sign[1];
    size_t signlen = 0;
    if (is_signed) {
        if (sv < 0) { sign[0] = '-'; signlen = 1; uv = (uint64_t)0 - (uint64_t)sv; }
        else if (sp->plus) { sign[0] = '+'; signlen = 1; uv = (uint64_t)sv; }
        else if (sp->space) { sign[0] = ' '; signlen = 1; uv = (uint64_t)sv; }
        else uv = (uint64_t)sv;
    }
    char digits[24];
    size_t n = rx_utoa(uv, base, upper, digits);
    size_t zero_prefix = 0;
    if (sp->prec >= 0) {
        if (sp->prec == 0 && uv == 0) n = 0;
        if ((size_t)sp->prec > n) zero_prefix = (size_t)sp->prec - n;
        sp->zero = false;   /* precision disables zero flag */
    }
    char prefix[3];
    size_t plen = 0;
    if (sp->alt && base == 16 && uv != 0) {
        prefix[0] = '0'; prefix[1] = upper ? 'X' : 'x'; plen = 2;
    }
    /* Prefix is emitted as part of the sign so zero padding lands after it */
    char signbuf[4];
    size_t sl = 0;
    for (size_t i = 0; i < signlen; i++) signbuf[sl++] = sign[i];
    for (size_t i = 0; i < plen; i++) signbuf[sl++] = prefix[i];
    rx_emit_number(k, sp, signbuf, sl, digits, n, zero_prefix);
}

/* ----------------------------------------------------------------------------
 * Exact floating point to decimal conversion.
 *
 * A finite double is mant * 2^e2. For a requested scale p (decimal places,
 * possibly negative) the printed integer is
 *     q = round_half_even(mant * 2^e2 * 10^p)
 * computed exactly with a small fixed-size big integer: numerator and
 * denominator are built from the positive and negative powers, then divided
 * bit-serially. This matches glibc's output digit for digit (glibc rounds
 * exact ties to even under the default rounding mode). Precision is capped at
 * RX_FMT_MAX_PREC digits; beyond that the request is clamped, not failed.
 * ------------------------------------------------------------------------- */

#define RX_FMT_MAX_PREC 40
#define RX_BIG_LIMBS 44           /* 1408 bits: 2^1074 * 10^40 fits with room */

typedef struct rx_big {
    uint32_t limb[RX_BIG_LIMBS];  /* little endian */
    int used;                     /* limbs in use (0 means value 0) */
} rx_big;

static void rx_big_set_u64(rx_big* b, uint64_t v) {
    for (int i = 0; i < RX_BIG_LIMBS; i++) b->limb[i] = 0;
    b->limb[0] = (uint32_t)v;
    b->limb[1] = (uint32_t)(v >> 32);
    b->used = b->limb[1] ? 2 : (b->limb[0] ? 1 : 0);
}

static void rx_big_mul_small(rx_big* b, uint32_t m) {
    uint64_t carry = 0;
    for (int i = 0; i < b->used; i++) {
        uint64_t t = (uint64_t)b->limb[i] * m + carry;
        b->limb[i] = (uint32_t)t;
        carry = t >> 32;
    }
    if (carry && b->used < RX_BIG_LIMBS) b->limb[b->used++] = (uint32_t)carry;
}

static void rx_big_shl(rx_big* b, int bits) {
    if (b->used == 0 || bits <= 0) return;
    int limbs = bits / 32, rem = bits % 32;
    if (limbs) {
        for (int i = RX_BIG_LIMBS - 1; i >= 0; i--) {
            b->limb[i] = (i - limbs >= 0) ? b->limb[i - limbs] : 0;
        }
        b->used += limbs;
        if (b->used > RX_BIG_LIMBS) b->used = RX_BIG_LIMBS;
    }
    if (rem) {
        uint32_t carry = 0;
        for (int i = 0; i < b->used; i++) {
            uint32_t nc = b->limb[i] >> (32 - rem);
            b->limb[i] = (b->limb[i] << rem) | carry;
            carry = nc;
        }
        if (carry && b->used < RX_BIG_LIMBS) b->limb[b->used++] = carry;
    }
}

static void rx_big_shl1_or(rx_big* b, uint32_t bit) {
    uint32_t carry = bit;
    for (int i = 0; i < b->used; i++) {
        uint32_t nc = b->limb[i] >> 31;
        b->limb[i] = (b->limb[i] << 1) | carry;
        carry = nc;
    }
    if (carry && b->used < RX_BIG_LIMBS) b->limb[b->used++] = carry;
    if (b->used == 0 && bit) { b->limb[0] = bit; b->used = 1; }
}

static int rx_big_cmp(const rx_big* a, const rx_big* b) {
    if (a->used != b->used) return a->used < b->used ? -1 : 1;
    for (int i = a->used - 1; i >= 0; i--) {
        if (a->limb[i] != b->limb[i]) return a->limb[i] < b->limb[i] ? -1 : 1;
    }
    return 0;
}

static void rx_big_sub(rx_big* a, const rx_big* b) {   /* a -= b, a >= b */
    uint64_t borrow = 0;
    for (int i = 0; i < a->used; i++) {
        uint64_t bi = (i < b->used) ? b->limb[i] : 0;
        uint64_t t = (uint64_t)a->limb[i] - bi - borrow;
        a->limb[i] = (uint32_t)t;
        borrow = (t >> 63) & 1;
    }
    while (a->used > 0 && a->limb[a->used - 1] == 0) a->used--;
}

static int rx_big_bit(const rx_big* b, int i) {
    return (int)((b->limb[i / 32] >> (i % 32)) & 1u);
}

static int rx_big_bits(const rx_big* b) {
    if (b->used == 0) return 0;
    uint32_t top = b->limb[b->used - 1];
    int n = 0;
    while (top) { n++; top >>= 1; }
    return (b->used - 1) * 32 + n;
}

static void rx_big_add_one(rx_big* b) {
    for (int i = 0; i < RX_BIG_LIMBS; i++) {
        if (++b->limb[i] != 0) {
            if (i >= b->used) b->used = i + 1;
            return;
        }
    }
}

/* q = round_half_even(num / den); den != 0 */
static void rx_big_divround(const rx_big* num, const rx_big* den, rx_big* q) {
    rx_big r;
    rx_big_set_u64(q, 0);
    rx_big_set_u64(&r, 0);
    for (int i = rx_big_bits(num) - 1; i >= 0; i--) {
        rx_big_shl1_or(&r, (uint32_t)rx_big_bit(num, i));
        if (rx_big_cmp(&r, den) >= 0) {
            rx_big_sub(&r, den);
            q->limb[i / 32] |= 1u << (i % 32);
            if (q->used < i / 32 + 1) q->used = i / 32 + 1;
        }
    }
    /* rounding: compare 2r with den */
    rx_big_shl1_or(&r, 0);
    int c = rx_big_cmp(&r, den);
    if (c > 0 || (c == 0 && (q->limb[0] & 1u))) rx_big_add_one(q);
}

/* Decimal string of b into out (cap bytes). Returns length. */
static size_t rx_big_to_dec(rx_big* b, char* out, size_t cap) {
    char tmp[512];
    size_t n = 0;
    if (b->used == 0) { if (cap) out[0] = '0'; return cap ? 1 : 0; }
    while (b->used > 0 && n + 9 < sizeof(tmp)) {
        /* divide by 10^9 */
        uint64_t rem = 0;
        for (int i = b->used - 1; i >= 0; i--) {
            uint64_t cur = (rem << 32) | b->limb[i];
            b->limb[i] = (uint32_t)(cur / 1000000000u);
            rem = cur % 1000000000u;
        }
        while (b->used > 0 && b->limb[b->used - 1] == 0) b->used--;
        for (int k = 0; k < 9; k++) {
            tmp[n++] = (char)('0' + rem % 10);
            rem /= 10;
            if (b->used == 0 && rem == 0) break;
        }
    }
    while (n > 1 && tmp[n - 1] == '0') n--;
    size_t len = n < cap ? n : cap;
    for (size_t i = 0; i < len; i++) out[i] = tmp[n - 1 - i];
    return len;
}

/* Digits of round(x * 10^p) for finite x >= 0. */
static size_t rx_scaled_digits(double x, int p, char* out, size_t cap) {
    uint64_t bits = rx_bits_of(x);
    int bexp = (int)((bits >> 52) & 0x7ff);
    uint64_t mant = bits & 0x000fffffffffffffull;
    int e2;
    if (bexp == 0) { e2 = -1074; }                     /* subnormal */
    else { mant |= 1ull << 52; e2 = bexp - 1075; }

    rx_big num, den, q;
    rx_big_set_u64(&num, mant);
    rx_big_set_u64(&den, 1);
    for (int i = 0; i < p; i++) rx_big_mul_small(&num, 10);
    for (int i = 0; i < -p; i++) rx_big_mul_small(&den, 10);
    if (e2 > 0) rx_big_shl(&num, e2);
    if (e2 < 0) rx_big_shl(&den, -e2);
    rx_big_divround(&num, &den, &q);
    return rx_big_to_dec(&q, out, cap);
}

/* Fixed notation body: integer digits, '.', prec fraction digits. */
static size_t rx_fmt_fixed(double x, int prec, char* out, size_t cap) {
    if (prec > RX_FMT_MAX_PREC) prec = RX_FMT_MAX_PREC;
    char d[400];
    size_t n = rx_scaled_digits(x, prec, d, sizeof(d));
    size_t pos = 0;
    if (n <= (size_t)prec) {
        /* value < 1: "0." followed by zero padding then the digits */
        if (pos < cap) out[pos++] = '0';
        if (prec > 0 && pos < cap) out[pos++] = '.';
        for (size_t i = 0; i < (size_t)prec - n && pos < cap; i++) out[pos++] = '0';
        for (size_t i = 0; i < n && pos < cap; i++) out[pos++] = d[i];
    } else {
        size_t ilen = n - (size_t)prec;
        for (size_t i = 0; i < ilen && pos < cap; i++) out[pos++] = d[i];
        if (prec > 0 && pos < cap) out[pos++] = '.';
        for (size_t i = ilen; i < n && pos < cap; i++) out[pos++] = d[i];
    }
    return pos;
}

/* Decimal exponent of x (> 0) after rounding to sig significant digits, and
 * those digits in d. */
static size_t rx_sig_digits(double x, int sig, int* e10, char* d, size_t cap) {
    /* estimate exponent */
    int e = 0;
    double t = x;
    while (t >= 10.0) { t /= 10.0; e++; }
    while (t < 1.0)   { t *= 10.0; e--; }
    for (int attempt = 0; attempt < 3; attempt++) {
        size_t n = rx_scaled_digits(x, sig - 1 - e, d, cap);
        if (n == (size_t)sig) { *e10 = e; return n; }
        if (n > (size_t)sig) e++; else e--;
    }
    *e10 = e;
    return rx_scaled_digits(x, sig - 1 - e, d, cap);
}

static size_t rx_put_exponent(int e, bool upper, char* out, size_t pos, size_t cap) {
    if (pos < cap) out[pos++] = upper ? 'E' : 'e';
    if (pos < cap) out[pos++] = e < 0 ? '-' : '+';
    unsigned ue = (unsigned)(e < 0 ? -e : e);
    char eb[8];
    size_t el = rx_utoa(ue, 10, false, eb);
    if (el < 2 && pos < cap) out[pos++] = '0';
    for (size_t i = 0; i < el && pos < cap; i++) out[pos++] = eb[i];
    return pos;
}

static size_t rx_fmt_exp(double x, int prec, bool upper, char* out, size_t cap) {
    if (prec > RX_FMT_MAX_PREC) prec = RX_FMT_MAX_PREC;
    char d[64];
    int e = 0;
    size_t n;
    if (x == 0.0) { n = 1; d[0] = '0'; for (int i = 1; i <= prec; i++) d[i] = '0'; n = (size_t)prec + 1; }
    else n = rx_sig_digits(x, prec + 1, &e, d, sizeof(d));
    size_t pos = 0;
    if (pos < cap) out[pos++] = d[0];
    if (prec > 0 && pos < cap) out[pos++] = '.';
    for (size_t i = 1; i < n && pos < cap; i++) out[pos++] = d[i];
    return rx_put_exponent(e, upper, out, pos, cap);
}

static size_t rx_strip_zeros(char* s, size_t n) {
    size_t dot = n, end = n;
    for (size_t i = 0; i < n; i++) {
        if (s[i] == '.') dot = i;
        if (s[i] == 'e' || s[i] == 'E') { end = i; break; }
    }
    if (dot == n) return n;
    size_t cut = end;
    while (cut > dot + 1 && s[cut - 1] == '0') cut--;
    if (cut == dot + 1) cut = dot;
    if (cut != end) {
        for (size_t i = end; i < n; i++) s[cut + (i - end)] = s[i];
        n -= end - cut;
    }
    return n;
}

static void rx_fmt_double(rx_sink* k, rx_spec* sp, double x, char conv) {
    char sign[1];
    size_t signlen = 0;
    bool upper = (conv == 'E' || conv == 'G' || conv == 'F');
    if (x != x) {
        sp->zero = false;
        rx_emit_number(k, sp, "", 0, upper ? "NAN" : "nan", 3, 0);
        return;
    }
    if (x < 0.0 || (x == 0.0 && 1.0 / x < 0.0)) { sign[0] = '-'; signlen = 1; x = -x; }
    else if (sp->plus) { sign[0] = '+'; signlen = 1; }
    else if (sp->space) { sign[0] = ' '; signlen = 1; }
    if (x == RX_INFINITY) {
        sp->zero = false;
        rx_emit_number(k, sp, sign, signlen, upper ? "INF" : "inf", 3, 0);
        return;
    }
    int prec = sp->prec < 0 ? 6 : sp->prec;
    char body[400];
    size_t n;
    char lc = (char)rx_tolower(conv);
    if (lc == 'f') {
        n = rx_fmt_fixed(x, prec, body, sizeof(body));
    } else if (lc == 'e') {
        n = rx_fmt_exp(x, prec, upper, body, sizeof(body));
    } else { /* g */
        if (prec == 0) prec = 1;
        if (prec > RX_FMT_MAX_PREC) prec = RX_FMT_MAX_PREC;
        int e = 0;
        if (x != 0.0) {
            char d[64];
            rx_sig_digits(x, prec, &e, d, sizeof(d));
        }
        if (e < -4 || e >= prec) {
            n = rx_fmt_exp(x, prec - 1, upper, body, sizeof(body));
        } else {
            n = rx_fmt_fixed(x, prec - 1 - e, body, sizeof(body));
        }
        if (!sp->alt) n = rx_strip_zeros(body, n);
    }
    rx_emit_number(k, sp, sign, signlen, body, n, 0);
}

static void rx_vformat(rx_sink* k, const char* fmt, va_list ap) {
    if (!fmt) fmt = "(null)";
    for (const char* p = fmt; *p; p++) {
        if (*p != '%') { rx_sink_putc(k, *p); continue; }
        p++;
        if (!*p) break;
        if (*p == '%') { rx_sink_putc(k, '%'); continue; }

        rx_spec sp = { false, false, false, false, false, 0, -1 };
        for (;; p++) {
            if (*p == '-') sp.left = true;
            else if (*p == '0') sp.zero = true;
            else if (*p == '+') sp.plus = true;
            else if (*p == ' ') sp.space = true;
            else if (*p == '#') sp.alt = true;
            else break;
        }
        if (*p == '*') {
            sp.width = va_arg(ap, int);
            if (sp.width < 0) { sp.left = true; sp.width = -sp.width; }
            p++;
        } else {
            while (rx_isdigit((unsigned char)*p)) {
                if (sp.width < 100000) sp.width = sp.width * 10 + (*p - '0');
                p++;
            }
        }
        if (*p == '.') {
            p++;
            sp.prec = 0;
            if (*p == '*') {
                sp.prec = va_arg(ap, int);
                if (sp.prec < 0) sp.prec = -1;
                p++;
            } else {
                while (rx_isdigit((unsigned char)*p)) {
                    if (sp.prec < 100000) sp.prec = sp.prec * 10 + (*p - '0');
                    p++;
                }
            }
        }
        int longs = 0;
        bool size_t_mod = false;
        for (;; p++) {
            if (*p == 'l') longs++;
            else if (*p == 'h') { /* accepted, treated as int */ }
            else if (*p == 'z' || *p == 'j' || *p == 't') size_t_mod = true;
            else break;
        }
        char conv = *p;
        if (!conv) break;
        switch (conv) {
            case 'd': case 'i': {
                int64_t v;
                if (longs >= 1) v = (longs >= 2) ? (int64_t)va_arg(ap, long long) : (int64_t)va_arg(ap, long);
                else if (size_t_mod) v = (int64_t)va_arg(ap, ptrdiff_t);
                else v = (int64_t)va_arg(ap, int);
                rx_fmt_integer(k, &sp, true, v, 0, 10, false);
                break;
            }
            case 'u': case 'x': case 'X': case 'o': {
                uint64_t v;
                if (longs >= 1) v = (longs >= 2) ? (uint64_t)va_arg(ap, unsigned long long) : (uint64_t)va_arg(ap, unsigned long);
                else if (size_t_mod) v = (uint64_t)va_arg(ap, size_t);
                else v = (uint64_t)va_arg(ap, unsigned int);
                unsigned base = conv == 'u' ? 10u : (conv == 'o' ? 8u : 16u);
                rx_fmt_integer(k, &sp, false, 0, v, base, conv == 'X');
                break;
            }
            case 'p': {
                uintptr_t v = (uintptr_t)va_arg(ap, void*);
                if (v == 0) {
                    rx_emit_number(k, &sp, "", 0, "(nil)", 5, 0);
                } else {
                    sp.alt = true;
                    rx_fmt_integer(k, &sp, false, 0, (uint64_t)v, 16, false);
                }
                break;
            }
            case 'c': {
                char c = (char)va_arg(ap, int);
                sp.zero = false;
                rx_emit_number(k, &sp, "", 0, &c, 1, 0);
                break;
            }
            case 's': {
                const char* s = va_arg(ap, const char*);
                if (!s) s = "(null)";
                size_t n = rx_strlen(s);
                if (sp.prec >= 0 && (size_t)sp.prec < n) n = (size_t)sp.prec;
                sp.zero = false;
                rx_emit_number(k, &sp, "", 0, s, n, 0);
                break;
            }
            case 'f': case 'F': case 'e': case 'E': case 'g': case 'G': {
                double v = va_arg(ap, double);
                rx_fmt_double(k, &sp, v, conv);
                break;
            }
            default:
                /* unknown conversion: echo it literally */
                rx_sink_putc(k, '%');
                rx_sink_putc(k, conv);
                break;
        }
    }
}

int rx_vsnprintf(char* buf, size_t cap, const char* fmt, va_list ap) {
    rx_sink k;
    k.buf = buf;
    k.cap = buf ? cap : 0;
    k.total = 0;
    k.fd = -1;
    k.staged = 0;
    k.error = 0;
    rx_vformat(&k, fmt, ap);
    if (buf && cap > 0) {
        size_t term = k.total < cap - 1 ? k.total : cap - 1;
        buf[term] = '\0';
    }
    return k.total > (size_t)INT32_MAX ? INT32_MAX : (int)k.total;
}

int rx_snprintf(char* buf, size_t cap, const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int n = rx_vsnprintf(buf, cap, fmt, ap);
    va_end(ap);
    return n;
}

int rx_vfdprintf(int fd, const char* fmt, va_list ap) {
    rx_sink k;
    k.buf = NULL;
    k.cap = 0;
    k.total = 0;
    k.fd = fd;
    k.staged = 0;
    k.error = 0;
    rx_vformat(&k, fmt, ap);
    rx_sink_flush(&k);
    if (k.error) return RX_PLAT_EIO;
    return k.total > (size_t)INT32_MAX ? INT32_MAX : (int)k.total;
}

int rx_fdprintf(int fd, const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int n = rx_vfdprintf(fd, fmt, ap);
    va_end(ap);
    return n;
}

int rx_printf(const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int n = rx_vfdprintf(RX_PLAT_STDOUT, fmt, ap);
    va_end(ap);
    return n;
}

int rx_eprintf(const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int n = rx_vfdprintf(RX_PLAT_STDERR, fmt, ap);
    va_end(ap);
    return n;
}

int64_t rx_write_all(int fd, const void* buf, size_t len) {
    if (!buf && len > 0) return RX_PLAT_EINVAL;
    const unsigned char* p = (const unsigned char*)buf;
    size_t done = 0;
    while (done < len) {
        int64_t w = rx_plat_write(fd, p + done, len - done);
        if (w < 0) return w;
        if (w == 0) return RX_PLAT_EIO;      /* no progress: avoid spinning */
        done += (size_t)w;
    }
    return (int64_t)done;
}

int64_t rx_fdputs(int fd, const char* s) {
    if (!s) s = "(null)";
    return rx_write_all(fd, s, rx_strlen(s));
}

/* ============================================================================
 * PRNG (xorshift64*)
 * ============================================================================ */

static uint64_t rx_rand_state = 0x9E3779B97F4A7C15ull;

void rx_srand(uint64_t seed) {
    rx_rand_state = seed ? seed : 0x9E3779B97F4A7C15ull;
}

int rx_rand(void) {
    uint64_t x = rx_rand_state;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    rx_rand_state = x;
    uint64_t r = x * 0x2545F4914F6CDD1Dull;
    return (int)((r >> 33) & (uint64_t)RX_RAND_MAX);
}

/* ============================================================================
 * Sorting (heapsort, in place, byte-wise swap)
 * ============================================================================ */

static void rx_swap_bytes(unsigned char* a, unsigned char* b, size_t size) {
    while (size--) {
        unsigned char t = *a;
        *a++ = *b;
        *b++ = t;
    }
}

static void rx_sift_down(unsigned char* base, size_t start, size_t end, size_t size,
                         int (*cmp)(const void*, const void*)) {
    size_t root = start;
    for (;;) {
        size_t child = root * 2 + 1;
        if (child >= end) return;
        size_t pick = child;
        if (child + 1 < end && cmp(base + child * size, base + (child + 1) * size) < 0) {
            pick = child + 1;
        }
        if (cmp(base + root * size, base + pick * size) >= 0) return;
        rx_swap_bytes(base + root * size, base + pick * size, size);
        root = pick;
    }
}

void rx_sort(void* base, size_t count, size_t size, int (*cmp)(const void*, const void*)) {
    if (!base || !cmp || count < 2 || size == 0) return;
    unsigned char* b = (unsigned char*)base;
    for (size_t i = count / 2; i-- > 0;) {
        rx_sift_down(b, i, count, size, cmp);
    }
    for (size_t end = count; end-- > 1;) {
        rx_swap_bytes(b, b + end * size, size);
        rx_sift_down(b, 0, end, size, cmp);
    }
}
