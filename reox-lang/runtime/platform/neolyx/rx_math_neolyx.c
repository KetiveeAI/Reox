/*
 * REOX Runtime - freestanding math shim for NeolyxOS
 *
 * Implements the rx_plat_* math functions from platform/rx_platform.h without
 * libm. The kernels are ports of the public-domain fdlibm / musl routines
 * (sqrt via the SSE2 instruction, floor/ceil/trunc/round/fmod by IEEE bit
 * manipulation, sin/cos/exp/log/atan/cbrt by their fdlibm polynomial
 * kernels), trimmed to double precision and plain C.
 *
 * Accuracy (checked against glibc on the development host with
 * `make PLATFORM=linux mathcheck`):
 *   sqrt                       exact (hardware sqrtsd)
 *   floor, ceil, trunc, round  exact
 *   fabs, fmod                 exact
 *   sin, cos                   <= 1 ulp for |x| <= 2^20 (Cody-Waite reduction
 *                              with a 2-part pi/2 plus correction); above
 *                              that the reduction loses bits
 *   tan                        sin/cos: <= 2 ulp away from the poles
 *   exp, log                   <= 1 ulp
 *   log2, log10                derived from log: <= 2 ulp
 *   atan, atan2                <= 1 ulp
 *   asin, acos                 via atan2 and sqrt: <= 2 ulp
 *   pow                        integer exponents with |e| <= 2^31 by binary
 *                              exponentiation (within 16 ulp of glibc for
 *                              |e| <= 20; each multiply adds <= 0.5 ulp),
 *                              otherwise exp(e*log(b)) with relative error
 *                              about |e*log(b)| * 2^-52 (measured 4e-15 for
 *                              |e| <= 10, b <= 10)
 *   cbrt                       <= 1 ulp
 * NaN and infinity inputs follow the C standard where fdlibm does.
 */

#include "../rx_platform.h"

typedef union { double d; uint64_t u; } rx_dbits;

static inline uint64_t rx_bits(double d) { rx_dbits b; b.d = d; return b.u; }
static inline double rx_from_bits(uint64_t u) { rx_dbits b; b.u = u; return b.d; }

#define RX_INF_D (__builtin_inf())
#define RX_NAN_D (__builtin_nan(""))

#define RX_M_PI       3.14159265358979323846
#define RX_M_PI_2     1.57079632679489661923
#define RX_M_PI_4     0.78539816339744830962
#define RX_M_LN2      0.69314718055994530942
#define RX_M_LN10     2.30258509299404568402

static inline bool rx_isnan(double x) { return x != x; }
static inline bool rx_isinf(double x) { return x == RX_INF_D || x == -RX_INF_D; }

/* ============================================================================
 * Exact operations
 * ============================================================================ */

double rx_plat_fabs(double x) {
    return rx_from_bits(rx_bits(x) & 0x7fffffffffffffffull);
}

double rx_plat_sqrt(double x) {
#if defined(__x86_64__) || defined(__SSE2__)
    double r;
    __asm__("sqrtsd %1, %0" : "=x"(r) : "x"(x));
    return r;
#else
    if (rx_isnan(x) || x < 0.0) return RX_NAN_D;
    if (x == 0.0 || rx_isinf(x)) return x;
    double g = x;
    for (int i = 0; i < 60; i++) {
        double n = 0.5 * (g + x / g);
        if (n == g) break;
        g = n;
    }
    return g;
#endif
}

/* musl trunc: operate on the exponent, exact for all inputs */
double rx_plat_trunc(double x) {
    uint64_t u = rx_bits(x);
    int e = (int)((u >> 52) & 0x7ff) - 0x3ff + 12;
    if (e >= 52 + 12) return x;             /* already integral, inf or nan */
    if (e < 12) e = 1;
    uint64_t m = (uint64_t)-1 >> e;
    if ((u & m) == 0) return x;
    return rx_from_bits(u & ~m);
}

double rx_plat_floor(double x) {
    if (rx_isnan(x) || rx_isinf(x)) return x;
    double t = rx_plat_trunc(x);
    if (t > x) t -= 1.0;
    if (t == 0.0 && x < 0.0) return -0.0;
    return t;
}

double rx_plat_ceil(double x) {
    if (rx_isnan(x) || rx_isinf(x)) return x;
    double t = rx_plat_trunc(x);
    if (t < x) t += 1.0;
    if (t == 0.0 && x < 0.0) return -0.0;
    return t;
}

double rx_plat_round(double x) {
    /* round half away from zero, like C round() */
    if (rx_isnan(x) || rx_isinf(x)) return x;
    uint64_t u = rx_bits(x);
    int e = (int)((u >> 52) & 0x7ff);
    if (e >= 0x3ff + 52) return x;          /* integral already */
    double a = rx_plat_fabs(x);
    double t = rx_plat_trunc(a);
    if (a - t >= 0.5) t += 1.0;
    if (t == 0.0 && x < 0.0) return -0.0;
    return x < 0.0 ? -t : t;
}

/* musl fmod: exact by exponent-aligned long division */
double rx_plat_fmod(double x, double y) {
    uint64_t ux = rx_bits(x), uy = rx_bits(y);
    int ex = (int)(ux >> 52 & 0x7ff);
    int ey = (int)(uy >> 52 & 0x7ff);
    int sx = (int)(ux >> 63);
    uint64_t i;

    if ((uy << 1) == 0 || rx_isnan(y) || ex == 0x7ff) return (x * y) / (x * y);
    if ((ux << 1) <= (uy << 1)) {
        if ((ux << 1) == (uy << 1)) return 0.0 * x;
        return x;
    }
    if (!ex) {
        for (i = ux << 12; (i >> 63) == 0; ex--, i <<= 1) {}
        ux <<= -ex + 1;
    } else {
        ux &= (uint64_t)-1 >> 12;
        ux |= 1ull << 52;
    }
    if (!ey) {
        for (i = uy << 12; (i >> 63) == 0; ey--, i <<= 1) {}
        uy <<= -ey + 1;
    } else {
        uy &= (uint64_t)-1 >> 12;
        uy |= 1ull << 52;
    }
    for (; ex > ey; ex--) {
        i = ux - uy;
        if ((i >> 63) == 0) {
            if (i == 0) return 0.0 * x;
            ux = i;
        }
        ux <<= 1;
    }
    i = ux - uy;
    if ((i >> 63) == 0) {
        if (i == 0) return 0.0 * x;
        ux = i;
    }
    for (; (ux >> 52) == 0; ux <<= 1, ex--) {}
    if (ex > 0) {
        ux -= 1ull << 52;
        ux |= (uint64_t)ex << 52;
    } else {
        ux >>= -ex + 1;
    }
    ux |= (uint64_t)sx << 63;
    return rx_from_bits(ux);
}

/* scalbn: multiply by 2^n without overflowing intermediate steps */
static double rx_scalbn(double x, int n) {
    double y = x;
    if (n > 1023) {
        y *= 0x1p1023; n -= 1023;
        if (n > 1023) {
            y *= 0x1p1023; n -= 1023;
            if (n > 1023) n = 1023;
        }
    } else if (n < -1022) {
        y *= 0x1p-1022 * 0x1p53; n += 1022 - 53;
        if (n < -1022) {
            y *= 0x1p-1022 * 0x1p53; n += 1022 - 53;
            if (n < -1022) n = -1022;
        }
    }
    return y * rx_from_bits((uint64_t)(0x3ff + n) << 52);
}

/* ============================================================================
 * exp / log  (fdlibm e_exp.c, e_log.c)
 * ============================================================================ */

double rx_plat_exp(double x) {
    static const double half[2] = { 0.5, -0.5 };
    static const double ln2hi = 6.93147180369123816490e-01;
    static const double ln2lo = 1.90821492927058770002e-10;
    static const double invln2 = 1.44269504088896338700e+00;
    static const double P1 = 1.66666666666666019037e-01;
    static const double P2 = -2.77777777770155933842e-03;
    static const double P3 = 6.61375632143793436117e-05;
    static const double P4 = -1.65339022054652515390e-06;
    static const double P5 = 4.13813679705723846039e-08;

    double hi, lo, c, xx, y;
    int k, sign;
    uint32_t hx = (uint32_t)(rx_bits(x) >> 32);
    sign = (int)(hx >> 31);
    hx &= 0x7fffffff;

    if (hx >= 0x4086232b) {                 /* |x| >= 708.39 */
        if (rx_isnan(x)) return x;
        if (x > 709.782712893384) return RX_INF_D;
        if (x < -745.13321910194110842) return 0.0;
    }
    if (hx > 0x3fd62e42) {                  /* |x| > 0.5 ln2 */
        if (hx >= 0x3ff0a2b2) {             /* |x| >= 1.5 ln2 */
            k = (int)(invln2 * x + half[sign]);
        } else {
            k = 1 - sign - sign;
        }
        hi = x - k * ln2hi;
        lo = k * ln2lo;
        x = hi - lo;
    } else if (hx > 0x3e300000) {           /* |x| > 2^-28 */
        k = 0;
        hi = x;
        lo = 0;
    } else {
        return 1 + x;
    }
    xx = x * x;
    c = x - xx * (P1 + xx * (P2 + xx * (P3 + xx * (P4 + xx * P5))));
    y = 1 + (x * c / (2 - c) - lo + hi);
    if (k == 0) return y;
    return rx_scalbn(y, k);
}

double rx_plat_log(double x) {
    static const double ln2_hi = 6.93147180369123816490e-01;
    static const double ln2_lo = 1.90821492927058770002e-10;
    static const double Lg1 = 6.666666666666735130e-01;
    static const double Lg2 = 3.999999999940941908e-01;
    static const double Lg3 = 2.857142874366239149e-01;
    static const double Lg4 = 2.222219843214978396e-01;
    static const double Lg5 = 1.818357216161805012e-01;
    static const double Lg6 = 1.531383769920937332e-01;
    static const double Lg7 = 1.479819860511658591e-01;

    uint64_t ix = rx_bits(x);
    uint32_t hx = (uint32_t)(ix >> 32);
    int k = 0;
    double hfsq, f, s, z, R, w, t1, t2, dk;

    if (hx < 0x00100000 || (hx >> 31)) {
        if ((ix << 1) == 0) return -RX_INF_D;        /* log(+-0) */
        if (hx >> 31) return RX_NAN_D;               /* log(-x) */
        k -= 54;                                     /* subnormal: scale up */
        x *= 0x1p54;
        ix = rx_bits(x);
        hx = (uint32_t)(ix >> 32);
    } else if (hx >= 0x7ff00000) {
        return x;                                    /* inf or nan */
    } else if (hx == 0x3ff00000 && (ix << 32) == 0) {
        return 0.0;                                  /* log(1) */
    }

    /* reduce x into [sqrt(2)/2, sqrt(2)] */
    hx += 0x3ff00000 - 0x3fe6a09e;
    k += (int)(hx >> 20) - 0x3ff;
    hx = (hx & 0x000fffff) + 0x3fe6a09e;
    ix = ((uint64_t)hx << 32) | (ix & 0xffffffffull);
    x = rx_from_bits(ix);

    f = x - 1.0;
    hfsq = 0.5 * f * f;
    s = f / (2.0 + f);
    z = s * s;
    w = z * z;
    t1 = w * (Lg2 + w * (Lg4 + w * Lg6));
    t2 = z * (Lg1 + w * (Lg3 + w * (Lg5 + w * Lg7)));
    R = t2 + t1;
    dk = k;
    return s * (hfsq + R) + dk * ln2_lo - hfsq + f + dk * ln2_hi;
}

double rx_plat_log2(double x)  { return rx_plat_log(x) / RX_M_LN2; }
double rx_plat_log10(double x) { return rx_plat_log(x) / RX_M_LN10; }

/* ============================================================================
 * pow
 * ============================================================================ */

static bool rx_is_integer_value(double x) {
    return !rx_isinf(x) && rx_plat_trunc(x) == x;
}

double rx_plat_pow(double b, double e) {
    if (e == 0.0) return 1.0;
    if (rx_isnan(b) || rx_isnan(e)) return RX_NAN_D;
    if (b == 1.0) return 1.0;

    /* integer exponents by binary exponentiation: exact where representable */
    if (rx_is_integer_value(e) && rx_plat_fabs(e) <= 2147483647.0) {
        long long n = (long long)e;
        bool neg = n < 0;
        unsigned long long m = neg ? (unsigned long long)(-n) : (unsigned long long)n;
        double r = 1.0, p = b;
        while (m) {
            if (m & 1ull) r *= p;
            p *= p;
            m >>= 1;
        }
        return neg ? 1.0 / r : r;
    }

    if (b == 0.0) return e > 0 ? 0.0 : RX_INF_D;
    if (rx_isinf(e)) {
        double ab = rx_plat_fabs(b);
        if (ab == 1.0) return 1.0;
        if ((ab > 1.0) == (e > 0)) return RX_INF_D;
        return 0.0;
    }
    if (rx_isinf(b)) return e > 0 ? RX_INF_D : 0.0;
    if (b < 0.0) return RX_NAN_D;           /* non-integer power of a negative */
    return rx_plat_exp(e * rx_plat_log(b));
}

/* ============================================================================
 * sin / cos / tan  (fdlibm k_sin.c, k_cos.c; Cody-Waite reduction)
 * ============================================================================ */

static double rx_k_sin(double x, double y, int iy) {
    static const double S1 = -1.66666666666666324348e-01;
    static const double S2 = 8.33333333332248946124e-03;
    static const double S3 = -1.98412698298579493134e-04;
    static const double S4 = 2.75573137070700676789e-06;
    static const double S5 = -2.50507602534068634195e-08;
    static const double S6 = 1.58969099521155010221e-10;
    double z = x * x;
    double w = z * z;
    double r = S2 + z * (S3 + z * S4) + z * w * (S5 + z * S6);
    double v = z * x;
    if (iy == 0) return x + v * (S1 + z * r);
    return x - ((z * (0.5 * y - v * r) - y) - v * S1);
}

static double rx_k_cos(double x, double y) {
    static const double C1 = 4.16666666666666019037e-02;
    static const double C2 = -1.38888888888741095749e-03;
    static const double C3 = 2.48015872894767294178e-05;
    static const double C4 = -2.75573143513906633035e-07;
    static const double C5 = 2.08757232129817482790e-09;
    static const double C6 = -1.13596475577881948265e-11;
    double z = x * x;
    double w = z * z;
    double r = z * (C1 + z * (C2 + z * C3)) + w * w * (C4 + z * (C5 + z * C6));
    double hz = 0.5 * z;
    w = 1.0 - hz;
    return w + (((1.0 - w) - hz) + (z * r - x * y));
}

/* Reduce x to r + y in [-pi/4, pi/4]; returns the quadrant (n mod 4).
 * This is the fdlibm medium-size path (pi/2 as a 2-part constant with a
 * second correction); it is exact to the last bit for |x| < 2^20 and
 * degrades gracefully beyond that. */
static int rx_rem_pio2(double x, double* r, double* y) {
    static const double invpio2 = 6.36619772367581382433e-01;
    static const double pio2_1  = 1.57079632673412561417e+00;  /* first 33 bits of pi/2 */
    static const double pio2_1t = 6.07710050650619224932e-11;  /* pi/2 - pio2_1 */
    static const double pio2_2  = 6.07710050630396597660e-11;  /* second 33 bits */
    static const double pio2_2t = 2.02226624879595063154e-21;  /* pi/2 - pio2_1 - pio2_2 */

    double fn = rx_plat_round(x * invpio2);
    double r0 = x - fn * pio2_1;         /* exact: fn has <= 20 bits, pio2_1 has 33 */
    double w = fn * pio2_1t;
    double y0 = r0 - w;                  /* first pass, good to about 85 bits */
    /* second pass with the split tail, as fdlibm does for medium arguments */
    double t = r0;
    w = fn * pio2_2;
    double r1 = t - w;
    w = fn * pio2_2t - ((t - r1) - w);
    y0 = r1 - w;
    double y1 = (r1 - y0) - w;
    *r = y0;
    *y = y1;
    long long q = (long long)fn;
    return (int)(q & 3);
}

double rx_plat_sin(double x) {
    if (rx_isnan(x) || rx_isinf(x)) return RX_NAN_D;
    if (rx_plat_fabs(x) <= RX_M_PI_4) {
        if (rx_plat_fabs(x) < 0x1p-26) return x;
        return rx_k_sin(x, 0.0, 0);
    }
    double r, y;
    int n = rx_rem_pio2(x, &r, &y);
    switch (n) {
        case 0:  return  rx_k_sin(r, y, 1);
        case 1:  return  rx_k_cos(r, y);
        case 2:  return -rx_k_sin(r, y, 1);
        default: return -rx_k_cos(r, y);
    }
}

double rx_plat_cos(double x) {
    if (rx_isnan(x) || rx_isinf(x)) return RX_NAN_D;
    if (rx_plat_fabs(x) <= RX_M_PI_4) {
        if (rx_plat_fabs(x) < 0x1p-27) return 1.0;
        return rx_k_cos(x, 0.0);
    }
    double r, y;
    int n = rx_rem_pio2(x, &r, &y);
    switch (n) {
        case 0:  return  rx_k_cos(r, y);
        case 1:  return -rx_k_sin(r, y, 1);
        case 2:  return -rx_k_cos(r, y);
        default: return  rx_k_sin(r, y, 1);
    }
}

double rx_plat_tan(double x) {
    if (rx_isnan(x) || rx_isinf(x)) return RX_NAN_D;
    double c = rx_plat_cos(x);
    double s = rx_plat_sin(x);
    if (c == 0.0) return s >= 0 ? RX_INF_D : -RX_INF_D;
    return s / c;
}

/* ============================================================================
 * atan / atan2 / asin / acos  (fdlibm s_atan.c, e_atan2.c)
 * ============================================================================ */

double rx_plat_atan(double x) {
    static const double atanhi[] = {
        4.63647609000806093515e-01, 7.85398163397448278999e-01,
        9.82793723247329054082e-01, 1.57079632679489655800e+00,
    };
    static const double atanlo[] = {
        2.26987774529616870924e-17, 3.06161699786838301793e-17,
        1.39033110312309984516e-17, 6.12323399573676603587e-17,
    };
    static const double aT[] = {
        3.33333333333329318027e-01, -1.99999999998764832476e-01,
        1.42857142725034663711e-01, -1.11111104054623557880e-01,
        9.09088713343650656196e-02, -7.69187620504482999495e-02,
        6.66107313738753120669e-02, -5.83357013379057348645e-02,
        4.97687799461593236017e-02, -3.65315727442169155270e-02,
        1.62858201153657823623e-02,
    };
    double w, s1, s2, z;
    uint32_t ix, sign;
    int id;

    ix = (uint32_t)(rx_bits(x) >> 32);
    sign = ix >> 31;
    ix &= 0x7fffffff;
    if (ix >= 0x44100000) {                 /* |x| >= 2^66 */
        if (rx_isnan(x)) return x;
        z = atanhi[3] + 0x1p-120;
        return sign ? -z : z;
    }
    if (ix < 0x3fdc0000) {                  /* |x| < 0.4375 */
        if (ix < 0x3e400000) return x;      /* |x| < 2^-27 */
        id = -1;
    } else {
        x = rx_plat_fabs(x);
        if (ix < 0x3ff30000) {              /* |x| < 1.1875 */
            if (ix < 0x3fe60000) {          /* 7/16 <= |x| < 11/16 */
                id = 0;
                x = (2.0 * x - 1.0) / (2.0 + x);
            } else {                        /* 11/16 <= |x| < 19/16 */
                id = 1;
                x = (x - 1.0) / (x + 1.0);
            }
        } else {
            if (ix < 0x40038000) {          /* |x| < 2.4375 */
                id = 2;
                x = (x - 1.5) / (1.0 + 1.5 * x);
            } else {                        /* 2.4375 <= |x| < 2^66 */
                id = 3;
                x = -1.0 / x;
            }
        }
    }
    z = x * x;
    w = z * z;
    s1 = z * (aT[0] + w * (aT[2] + w * (aT[4] + w * (aT[6] + w * (aT[8] + w * aT[10])))));
    s2 = w * (aT[1] + w * (aT[3] + w * (aT[5] + w * (aT[7] + w * aT[9]))));
    if (id < 0) return x - x * (s1 + s2);
    z = atanhi[id] - (x * (s1 + s2) - atanlo[id] - x);
    return sign ? -z : z;
}

double rx_plat_atan2(double y, double x) {
    static const double pi_lo = 1.2246467991473531772E-16;
    if (rx_isnan(x) || rx_isnan(y)) return x + y;
    uint64_t ux = rx_bits(x), uy = rx_bits(y);
    uint32_t ix = (uint32_t)(ux >> 32), lx = (uint32_t)ux;
    uint32_t iy = (uint32_t)(uy >> 32), ly = (uint32_t)uy;
    if (((ix - 0x3ff00000) | lx) == 0) return rx_plat_atan(y);   /* x = 1 */
    uint32_t m = ((iy >> 31) & 1) | ((ix >> 30) & 2);           /* 2*sign(x)+sign(y) */
    ix &= 0x7fffffff;
    iy &= 0x7fffffff;

    if ((iy | ly) == 0) {                   /* y = 0 */
        switch (m) {
            case 0: case 1: return y;
            case 2: return RX_M_PI;
            default: return -RX_M_PI;
        }
    }
    if ((ix | lx) == 0) return (m & 1) ? -RX_M_PI_2 : RX_M_PI_2;   /* x = 0 */
    if (ix == 0x7ff00000) {                 /* x = +-inf */
        if (iy == 0x7ff00000) {
            switch (m) {
                case 0: return RX_M_PI_4;
                case 1: return -RX_M_PI_4;
                case 2: return 3.0 * RX_M_PI_4;
                default: return -3.0 * RX_M_PI_4;
            }
        }
        switch (m) {
            case 0: return 0.0;
            case 1: return -0.0;
            case 2: return RX_M_PI;
            default: return -RX_M_PI;
        }
    }
    if (ix + (64 << 20) < iy || iy == 0x7ff00000) {     /* |y/x| > 2^64 */
        return (m & 1) ? -RX_M_PI_2 : RX_M_PI_2;
    }
    double z;
    if ((m & 2) && iy + (64 << 20) < ix) {   /* |y/x| < 2^-64, x < 0 */
        z = 0.0;
    } else {
        z = rx_plat_atan(rx_plat_fabs(y / x));
    }
    switch (m) {
        case 0: return z;
        case 1: return -z;
        case 2: return RX_M_PI - (z - pi_lo);
        default: return (z - pi_lo) - RX_M_PI;
    }
}

double rx_plat_asin(double x) {
    if (rx_isnan(x)) return x;
    if (x > 1.0 || x < -1.0) return RX_NAN_D;
    if (x == 1.0) return RX_M_PI_2;
    if (x == -1.0) return -RX_M_PI_2;
    return rx_plat_atan2(x, rx_plat_sqrt((1.0 - x) * (1.0 + x)));
}

double rx_plat_acos(double x) {
    if (rx_isnan(x)) return x;
    if (x > 1.0 || x < -1.0) return RX_NAN_D;
    if (x == 1.0) return 0.0;
    if (x == -1.0) return RX_M_PI;
    return rx_plat_atan2(rx_plat_sqrt((1.0 - x) * (1.0 + x)), x);
}

/* ============================================================================
 * cbrt  (fdlibm s_cbrt.c)
 * ============================================================================ */

double rx_plat_cbrt(double x) {
    static const uint32_t B1 = 715094163;
    static const uint32_t B2 = 696219795;
    static const double P0 = 1.87595182427177009643;
    static const double P1 = -1.88497979543377169875;
    static const double P2 = 1.621429720105354466140;
    static const double P3 = -0.758397934778766047437;
    static const double P4 = 0.145996192886612446982;

    uint64_t u = rx_bits(x);
    uint32_t hx = (uint32_t)(u >> 32) & 0x7fffffff;
    double r, s, t, w;

    if (hx >= 0x7ff00000) return x + x;     /* inf or nan */
    if (hx < 0x00100000) {                  /* zero or subnormal */
        if ((u << 1) == 0) return x;
        t = x * 0x1p54;
        uint64_t ut = rx_bits(t);
        hx = (uint32_t)(ut >> 32) & 0x7fffffff;
        hx = hx / 3 + B2;
        ut = (ut & (1ull << 63)) | ((uint64_t)hx << 32);
        t = rx_from_bits(ut);
    } else {
        hx = hx / 3 + B1;
        uint64_t ut = (u & (1ull << 63)) | ((uint64_t)hx << 32);
        t = rx_from_bits(ut);
    }
    r = (t * t) * (t / x);
    t = t * ((P0 + r * (P1 + r * P2)) + ((r * r) * r) * (P3 + r * P4));
    uint64_t ut = rx_bits(t);
    ut = (ut + 0x80000000ull) & 0xffffffffc0000000ull;
    t = rx_from_bits(ut);
    s = t * t;
    r = x / s;
    w = t + t;
    r = (r - t) / (w + r);
    t = t + t * r;
    return t;
}
