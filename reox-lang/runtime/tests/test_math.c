/*
 * Host-side accuracy check for platform/neolyx/rx_math_neolyx.c.
 * The freestanding math file is compiled on the Linux host and compared
 * against glibc's libm in ulps. The bounds asserted here are the ones
 * documented at the top of rx_math_neolyx.c.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <stdint.h>
#include "../platform/rx_platform.h"

static int failures = 0;

static double ulps(double got, double want) {
    if (got == want) return 0.0;
    if (isnan(got) && isnan(want)) return 0.0;
    if (isnan(got) || isnan(want) || isinf(got) || isinf(want)) return 1e30;
    uint64_t a, b;
    memcpy(&a, &got, 8); memcpy(&b, &want, 8);
    if ((a >> 63) != (b >> 63)) return (got == 0.0 || want == 0.0) ? 1.0 : 1e30;
    return (double)(a > b ? a - b : b - a);
}

static uint64_t rng = 0x12345678abcdefULL;
static double urand(void) {
    rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
    return (double)(rng >> 11) / 9007199254740992.0;
}

typedef double (*f1)(double);
static void check1(const char* name, f1 got, f1 want, double lo, double hi, double max_ulp, int n) {
    double worst = 0;
    double worst_x = 0;
    for (int i = 0; i < n; i++) {
        double x = lo + (hi - lo) * urand();
        double u = ulps(got(x), want(x));
        if (u > worst) { worst = u; worst_x = x; }
    }
    /* a few special values */
    const double specials[] = { 0.0, -0.0, 1.0, -1.0, 0.5, 2.0, 1e-300, 1e300, 3.0 };
    for (size_t i = 0; i < sizeof specials / sizeof *specials; i++) {
        double x = specials[i];
        if (x < lo || x > hi) continue;
        double u = ulps(got(x), want(x));
        if (u > worst) { worst = u; worst_x = x; }
    }
    if (worst > max_ulp) { printf("FAIL %-6s worst %.0f ulp at %.17g (limit %.0f)\n", name, worst, worst_x, max_ulp); failures++; }
    else printf("ok   %-6s worst %.0f ulp over [%g, %g]\n", name, worst, lo, hi);
}

static double w_round(double x) { return round(x); }
static double w_cbrt(double x) { return (double)cbrtl((long double)x); }

int main(void) {
    const int N = 200000;
    check1("sqrt",  rx_plat_sqrt,  sqrt,  0, 1e6, 0, N);
    check1("floor", rx_plat_floor, floor, -1e6, 1e6, 0, N);
    check1("ceil",  rx_plat_ceil,  ceil,  -1e6, 1e6, 0, N);
    check1("trunc", rx_plat_trunc, trunc, -1e6, 1e6, 0, N);
    check1("round", rx_plat_round, w_round, -1e6, 1e6, 0, N);
    check1("fabs",  rx_plat_fabs,  fabs,  -1e6, 1e6, 0, N);
    check1("sin",   rx_plat_sin,   sin,   -1e5, 1e5, 1, N);
    check1("cos",   rx_plat_cos,   cos,   -1e5, 1e5, 1, N);
    check1("sin20", rx_plat_sin,   sin,   -1048576, 1048576, 1, N);
    check1("tan",   rx_plat_tan,   tan,   -1.5, 1.5, 2, N);
    check1("exp",   rx_plat_exp,   exp,   -700, 700, 1, N);
    check1("log",   rx_plat_log,   log,   1e-300, 1e300, 1, N);
    check1("log2",  rx_plat_log2,  log2,  1e-300, 1e300, 2, N);
    check1("log10", rx_plat_log10, log10, 1e-300, 1e300, 2, N);
    check1("atan",  rx_plat_atan,  atan,  -1e6, 1e6, 1, N);
    check1("asin",  rx_plat_asin,  asin,  -1, 1, 2, N);
    check1("acos",  rx_plat_acos,  acos,  -1, 1, 2, N);
    check1("cbrt",  rx_plat_cbrt,  w_cbrt, -1e6, 1e6, 1, N);

    /* two-argument functions */
    double worst = 0;
    for (int i = 0; i < N; i++) {
        double y = -100 + 200 * urand(), x = -100 + 200 * urand();
        double u = ulps(rx_plat_atan2(y, x), atan2(y, x));
        if (u > worst) worst = u;
    }
    printf("%s atan2  worst %.0f ulp\n", worst > 1 ? "FAIL" : "ok  ", worst); if (worst > 1) failures++;

    worst = 0;
    for (int i = 0; i < N; i++) {
        double x = -1e6 + 2e6 * urand(), y = -1e3 + 2e3 * urand();
        if (y == 0) continue;
        double u = ulps(rx_plat_fmod(x, y), fmod(x, y));
        if (u > worst) worst = u;
    }
    printf("%s fmod   worst %.0f ulp\n", worst > 0 ? "FAIL" : "ok  ", worst); if (worst > 0) failures++;

    /* pow: integer exponents exact-ish (glibc rounds the final product, we
     * multiply step by step, so allow a few ulps), real exponents relative */
    worst = 0;
    for (int i = 0; i < N; i++) {
        double b = 0.1 + 10 * urand();
        int e = (int)(-20 + 40 * urand());
        double u = ulps(rx_plat_pow(b, e), pow(b, e));
        if (u > worst) worst = u;
    }
    printf("%s pow(int) worst %.0f ulp\n", worst > 16 ? "FAIL" : "ok  ", worst); if (worst > 16) failures++;
    double worst_rel = 0;
    for (int i = 0; i < N; i++) {
        double b = 0.1 + 10 * urand(), e = -10 + 20 * urand();
        double g = rx_plat_pow(b, e), w = pow(b, e);
        double rel = fabs(g - w) / fabs(w);
        if (rel > worst_rel) worst_rel = rel;
    }
    printf("%s pow(real) worst rel %.3g\n", worst_rel > 1e-13 ? "FAIL" : "ok  ", worst_rel); if (worst_rel > 1e-13) failures++;

    /* special cases */
    if (!isnan(rx_plat_sqrt(-1)) || !isnan(rx_plat_log(-1)) || rx_plat_log(0) != -INFINITY ||
        rx_plat_exp(1000) != INFINITY || rx_plat_exp(-1000) != 0 || rx_plat_pow(0, 0) != 1 ||
        !isnan(rx_plat_sin(INFINITY)) || rx_plat_atan2(0, -1) != 3.14159265358979323846) {
        puts("FAIL special cases"); failures++;
    }

    printf("%s (%d failures)\n", failures ? "test_math: FAILED" : "test_math: ok", failures);
    return failures ? 1 : 0;
}
