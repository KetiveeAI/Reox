/*
 * REOX System Stdlib Wrappers
 * Maps system.rx stdlib declarations onto the runtime platform contract.
 *
 * Header-only: every function is static inline so generated programs pick up
 * exactly what they use. Nothing here touches libc; all I/O goes through
 * rx_plat_* so the same header serves Linux and NeolyxOS. Where NeolyxOS has
 * no syscall yet (directories, environment, exec) the platform layer returns
 * RX_PLAT_ENOSYS and the wrapper reports failure.
 *
 * Copyright (c) 2025-2026 KetiveeAI
 */

#ifndef REOX_SYSTEM_STDLIB_H
#define REOX_SYSTEM_STDLIB_H

#include "reox_runtime.h"
#include "../platform/rx_platform.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ============================================================================
 * File Operations (from system.rx)
 * ============================================================================ */

/* file_read(path: string) -> string
 * Returns a heap buffer the caller frees with rx_free, or NULL on failure. */
static inline const char* file_read(const char* path) {
    if (!path) return NULL;

    int fd = rx_plat_open(path, RX_PLAT_O_RDONLY);
    if (fd < 0) return NULL;

    int64_t size = rx_plat_seek(fd, 0, RX_PLAT_SEEK_END);
    if (size < 0 || rx_plat_seek(fd, 0, RX_PLAT_SEEK_SET) < 0) {
        rx_plat_close(fd);
        return NULL;
    }
    if ((uint64_t)size >= SIZE_MAX - 1) {
        rx_plat_close(fd);
        return NULL;
    }

    char* buffer = (char*)rx_alloc((size_t)size + 1);
    if (!buffer) {
        rx_plat_close(fd);
        return NULL;
    }

    size_t got = 0;
    while (got < (size_t)size) {
        int64_t n = rx_plat_read(fd, buffer + got, (size_t)size - got);
        if (n < 0) {
            rx_free(buffer);
            rx_plat_close(fd);
            return NULL;
        }
        if (n == 0) break;   /* file shrank while reading */
        got += (size_t)n;
    }
    buffer[got] = '\0';
    rx_plat_close(fd);
    return buffer;
}

static inline int64_t rx_sys_write_whole(const char* path, const char* content, uint32_t flags) {
    if (!path || !content) return 0;
    int fd = rx_plat_open(path, flags);
    if (fd < 0) return 0;
    size_t len = rx_strlen(content);
    int64_t written = rx_write_all(fd, content, len);
    int rc = rx_plat_close(fd);
    return (written == (int64_t)len && rc == RX_PLAT_OK) ? 1 : 0;
}

/* file_write(path: string, content: string) -> bool */
static inline int64_t file_write(const char* path, const char* content) {
    return rx_sys_write_whole(path, content, RX_PLAT_O_WRONLY | RX_PLAT_O_CREAT | RX_PLAT_O_TRUNC);
}

/* file_append(path: string, content: string) -> bool */
static inline int64_t file_append(const char* path, const char* content) {
    return rx_sys_write_whole(path, content, RX_PLAT_O_WRONLY | RX_PLAT_O_CREAT | RX_PLAT_O_APPEND);
}

/* file_exists(path: string) -> bool */
static inline int64_t file_exists(const char* path) {
    rx_plat_stat_t st;
    if (!path) return 0;
    return rx_plat_stat(path, &st) == RX_PLAT_OK ? 1 : 0;
}

/* file_delete(path: string) -> bool */
static inline int64_t file_delete(const char* path) {
    if (!path) return 0;
    return rx_plat_unlink(path) == RX_PLAT_OK ? 1 : 0;
}

/* file_size(path: string) -> int (-1 on failure) */
static inline int64_t file_size(const char* path) {
    rx_plat_stat_t st;
    if (!path) return -1;
    if (rx_plat_stat(path, &st) != RX_PLAT_OK) return -1;
    if (st.size > (uint64_t)INT64_MAX) return -1;
    return (int64_t)st.size;
}

/* file_copy(src: string, dest: string) -> bool */
static inline int64_t file_copy(const char* src, const char* dest) {
    if (!src || !dest) return 0;

    int in = rx_plat_open(src, RX_PLAT_O_RDONLY);
    if (in < 0) return 0;

    int out = rx_plat_open(dest, RX_PLAT_O_WRONLY | RX_PLAT_O_CREAT | RX_PLAT_O_TRUNC);
    if (out < 0) {
        rx_plat_close(in);
        return 0;
    }

    char buffer[4096];
    int64_t ok = 1;
    for (;;) {
        int64_t n = rx_plat_read(in, buffer, sizeof(buffer));
        if (n < 0) { ok = 0; break; }
        if (n == 0) break;
        if (rx_write_all(out, buffer, (size_t)n) != n) { ok = 0; break; }
    }

    if (rx_plat_close(in) != RX_PLAT_OK) ok = 0;
    if (rx_plat_close(out) != RX_PLAT_OK) ok = 0;
    return ok;
}

/* file_rename(old_path: string, new_path: string) -> bool */
static inline int64_t file_rename(const char* old_path, const char* new_path) {
    if (!old_path || !new_path) return 0;
    return rx_plat_rename(old_path, new_path) == RX_PLAT_OK ? 1 : 0;
}

/* ============================================================================
 * Directory Operations
 * ============================================================================ */

/* dir_create(path: string) -> bool */
static inline int64_t dir_create(const char* path) {
    if (!path) return 0;
    return rx_plat_mkdir(path) == RX_PLAT_OK ? 1 : 0;
}

/* dir_exists(path: string) -> bool */
static inline int64_t dir_exists(const char* path) {
    rx_plat_stat_t st;
    if (!path) return 0;
    if (rx_plat_stat(path, &st) != RX_PLAT_OK) return 0;
    return st.kind == RX_PLAT_KIND_DIR ? 1 : 0;
}

/* dir_delete(path: string) -> bool */
static inline int64_t dir_delete(const char* path) {
    if (!path) return 0;
    return rx_plat_rmdir(path) == RX_PLAT_OK ? 1 : 0;
}

/* ============================================================================
 * Environment Variables
 * ============================================================================ */

/* env_get(name: string) -> string
 * Returns a pointer into a static buffer (valid until the next call), or NULL
 * when the variable is unset or the platform has no environment. */
static inline const char* env_get(const char* name) {
    static char buffer[4096];
    if (!name) return NULL;
    if (rx_plat_getenv(name, buffer, sizeof(buffer)) != RX_PLAT_OK) return NULL;
    return buffer;
}

/* env_set(name: string, value: string) -> bool */
static inline int64_t env_set(const char* name, const char* value) {
    if (!name || !value) return 0;
    return rx_plat_setenv(name, value) == RX_PLAT_OK ? 1 : 0;
}

/* ============================================================================
 * Process Control
 * ============================================================================ */

/* Command line arguments storage */
static struct {
    int argc;
    char** argv;
} rx_sys_args = { 0, NULL };

/* Initialize with main() args */
static inline void sys_init_args(int argc, char** argv) {
    if (argc < 0 || (argc > 0 && !argv)) { argc = 0; argv = NULL; }
    rx_sys_args.argc = argc;
    rx_sys_args.argv = argv;
}

/* sys_args() -> Array */
static inline int64_t sys_args_count(void) {
    return rx_sys_args.argc;
}

static inline const char* sys_args_get(int64_t index) {
    if (index < 0 || index >= rx_sys_args.argc || !rx_sys_args.argv) return NULL;
    return rx_sys_args.argv[index];
}

/* sys_exit(code: int) -> void */
static inline void sys_exit_rx(int64_t code) {
    rx_plat_exit((int)code);
}

/* sys_exec(command: string) -> int (exit status, or -1) */
static inline int64_t sys_exec(const char* command) {
    if (!command) return -1;
    int rc = rx_plat_exec(command);
    return rc < 0 ? -1 : (int64_t)rc;
}

/* sys_pid() -> int */
static inline int64_t sys_pid(void) {
    int64_t pid = rx_plat_getpid();
    return pid < 0 ? -1 : pid;
}

/* sys_sleep(ms: int) -> void */
static inline void sys_sleep(int64_t ms) {
    if (ms > 0) (void)rx_plat_sleep_ms((uint64_t)ms);
}

/* ============================================================================
 * Time Functions
 * ============================================================================ */

/* time_now() -> int (Unix timestamp in seconds, -1 when unavailable) */
static inline int64_t time_now(void) {
    int64_t t = rx_plat_time_unix();
    return t < 0 ? -1 : t;
}

/* time_ms() -> int (monotonic milliseconds; NeolyxOS counts from boot) */
static inline int64_t time_ms(void) {
    return (int64_t)rx_plat_time_ms();
}

/* Civil-from-days (Howard Hinnant's algorithm), proleptic Gregorian, UTC. */
static inline void rx_sys_civil(int64_t days, int64_t* y, unsigned* m, unsigned* d) {
    days += 719468;
    int64_t era = (days >= 0 ? days : days - 146096) / 146097;
    unsigned doe = (unsigned)(days - era * 146097);
    unsigned yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    int64_t yy = (int64_t)yoe + era * 400;
    unsigned doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    unsigned mp = (5 * doy + 2) / 153;
    *d = doy - (153 * mp + 2) / 5 + 1;
    *m = mp < 10 ? mp + 3 : mp - 9;
    *y = yy + (*m <= 2 ? 1 : 0);
}

/* time_format(timestamp: int, format: string) -> string
 * UTC only (NeolyxOS has no time zone database). Supports %Y %m %d %H %M %S
 * %y %j %% ; any other directive is copied literally. Returns a static
 * buffer valid until the next call. */
static inline const char* time_format(int64_t timestamp, const char* format) {
    static char buffer[128];
    if (!format) format = "%Y-%m-%d %H:%M:%S";

    int64_t days = timestamp / 86400;
    int64_t rem = timestamp % 86400;
    if (rem < 0) { rem += 86400; days -= 1; }
    int64_t year; unsigned month, day;
    rx_sys_civil(days, &year, &month, &day);
    unsigned hour = (unsigned)(rem / 3600);
    unsigned minute = (unsigned)((rem % 3600) / 60);
    unsigned second = (unsigned)(rem % 60);

    size_t n = 0;
    for (const char* p = format; *p && n + 8 < sizeof(buffer); p++) {
        if (*p != '%' || !p[1]) { buffer[n++] = *p; continue; }
        p++;
        int w;
        switch (*p) {
            case 'Y': w = rx_snprintf(buffer + n, sizeof(buffer) - n, "%lld", (long long)year); break;
            case 'y': w = rx_snprintf(buffer + n, sizeof(buffer) - n, "%02lld", (long long)(year % 100)); break;
            case 'm': w = rx_snprintf(buffer + n, sizeof(buffer) - n, "%02u", month); break;
            case 'd': w = rx_snprintf(buffer + n, sizeof(buffer) - n, "%02u", day); break;
            case 'H': w = rx_snprintf(buffer + n, sizeof(buffer) - n, "%02u", hour); break;
            case 'M': w = rx_snprintf(buffer + n, sizeof(buffer) - n, "%02u", minute); break;
            case 'S': w = rx_snprintf(buffer + n, sizeof(buffer) - n, "%02u", second); break;
            case '%': buffer[n] = '%'; w = 1; break;
            default:  buffer[n] = '%'; buffer[n + 1] = *p; w = 2; break;
        }
        if (w < 0) break;
        n += (size_t)w;
    }
    buffer[n < sizeof(buffer) ? n : sizeof(buffer) - 1] = '\0';
    return buffer;
}

/* ============================================================================
 * Path Utilities
 * ============================================================================ */

/* path_join(a: string, b: string) -> string (heap, free with rx_free) */
static inline const char* path_join(const char* a, const char* b) {
    if (!a || !b) return NULL;

    size_t len_a = rx_strlen(a);
    size_t len_b = rx_strlen(b);
    if (len_a > SIZE_MAX - len_b - 2) return NULL;

    char* result = (char*)rx_alloc(len_a + len_b + 2);
    if (!result) return NULL;

    rx_memcpy(result, a, len_a);
    size_t n = len_a;
    if (len_a > 0 && a[len_a - 1] != '/') result[n++] = '/';
    rx_memcpy(result + n, b, len_b);
    result[n + len_b] = '\0';
    return result;
}

/* path_basename(path: string) -> string (points into path) */
static inline const char* path_basename(const char* path) {
    if (!path) return NULL;
    const char* last = rx_strrchr(path, '/');
    return last ? last + 1 : path;
}

/* path_dirname(path: string) -> string (heap when a slash is present) */
static inline const char* path_dirname(const char* path) {
    if (!path) return NULL;
    const char* last = rx_strrchr(path, '/');
    if (!last) return ".";
    return rx_strndup(path, (size_t)(last - path));
}

/* path_extension(path: string) -> string */
static inline const char* path_extension(const char* path) {
    if (!path) return NULL;
    const char* basename = path_basename(path);
    const char* dot = rx_strrchr(basename, '.');
    return dot ? dot + 1 : "";
}

/* cwd() -> string (static buffer, NULL when unsupported) */
static inline const char* cwd(void) {
    static char buffer[4096];
    if (rx_plat_getcwd(buffer, sizeof(buffer)) != RX_PLAT_OK) return NULL;
    return buffer;
}

/* chdir_rx(path: string) -> bool */
static inline int64_t chdir_rx(const char* path) {
    if (!path) return 0;
    return rx_plat_chdir(path) == RX_PLAT_OK ? 1 : 0;
}

#ifdef __cplusplus
}
#endif

#endif /* REOX_SYSTEM_STDLIB_H */
