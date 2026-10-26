/*
 * REOX Runtime - Platform Contract
 *
 * This header is the single boundary between the OS-independent runtime
 * (runtime/core/) and the operating system. Every allocation, every byte of
 * I/O, every clock read, every math primitive and every framebuffer or input
 * access made by core goes through one of the rx_plat_* functions declared
 * here.
 *
 * Implementations:
 *   platform/linux/rx_platform_linux.c    - libc, libm, optional SDL2
 *   platform/neolyx/rx_platform_neolyx.c  - nxsyscall.h syscalls + nx_malloc
 *   platform/neolyx/rx_math_neolyx.c      - freestanding libm subset
 *
 * Rules for implementers:
 *   - Every function must be safe to call with NULL / zero-size arguments and
 *     must report failure through its return value; none may crash.
 *   - Functions returning int return RX_PLAT_OK (0) on success or a negative
 *     RX_PLAT_E* code on failure.
 *   - Functions returning a pointer return NULL on failure.
 *   - No implementation may call back into core.
 *
 * Only <stdint.h>, <stddef.h> and <stdbool.h> may be included here so that
 * the header is usable from a freestanding translation unit.
 */

#ifndef RX_PLATFORM_H
#define RX_PLATFORM_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

#if defined(__GNUC__)
#define RX_PLAT_NORETURN __attribute__((noreturn))
#else
#define RX_PLAT_NORETURN
#endif

/* ============================================================================
 * Error codes
 * ============================================================================ */

enum {
    RX_PLAT_OK      =  0,
    RX_PLAT_EINVAL  = -1,   /* bad argument (NULL pointer, bad fd, bad size) */
    RX_PLAT_ENOMEM  = -2,   /* out of memory */
    RX_PLAT_EIO     = -3,   /* I/O failure reported by the OS */
    RX_PLAT_ENOENT  = -4,   /* path does not exist */
    RX_PLAT_ENOSYS  = -5,   /* not supported on this platform */
    RX_PLAT_EBADF   = -6,   /* descriptor is not open */
    RX_PLAT_EEXIST  = -7,   /* path already exists */
    RX_PLAT_ERANGE  = -8    /* buffer too small */
};

/* ============================================================================
 * Memory
 * ============================================================================ */

/* Allocate size bytes, 16-byte aligned. Returns NULL when size is 0 or the
 * allocation fails. Contents are undefined. */
void* rx_plat_alloc(size_t size);

/* Resize a block returned by rx_plat_alloc/rx_plat_realloc. On failure the
 * original block is untouched and NULL is returned. ptr == NULL behaves like
 * rx_plat_alloc(size). size == 0 frees ptr and returns NULL. */
void* rx_plat_realloc(void* ptr, size_t size);

/* Release a block. NULL is ignored. */
void rx_plat_free(void* ptr);

/* ============================================================================
 * Byte I/O
 * ============================================================================ */

#define RX_PLAT_STDIN   0
#define RX_PLAT_STDOUT  1
#define RX_PLAT_STDERR  2

/* Open flags. Values match both POSIX (Linux) and nxsyscall.h (NeolyxOS). */
#define RX_PLAT_O_RDONLY  0x0000u
#define RX_PLAT_O_WRONLY  0x0001u
#define RX_PLAT_O_RDWR    0x0002u
#define RX_PLAT_O_CREAT   0x0040u
#define RX_PLAT_O_TRUNC   0x0200u
#define RX_PLAT_O_APPEND  0x0400u

/* Seek origins. Values match both POSIX and nxsyscall.h. */
#define RX_PLAT_SEEK_SET  0
#define RX_PLAT_SEEK_CUR  1
#define RX_PLAT_SEEK_END  2

/* Write up to len bytes. Returns bytes written (may be short) or a negative
 * RX_PLAT_E* code. len == 0 returns 0. */
int64_t rx_plat_write(int fd, const void* buf, size_t len);

/* Read up to len bytes. Returns bytes read, 0 at end of file, or a negative
 * RX_PLAT_E* code. */
int64_t rx_plat_read(int fd, void* buf, size_t len);

/* Open a path. Returns a descriptor >= 0 or a negative RX_PLAT_E* code. */
int rx_plat_open(const char* path, uint32_t flags);

/* Close a descriptor. */
int rx_plat_close(int fd);

/* Reposition. Returns the new absolute offset or a negative code. */
int64_t rx_plat_seek(int fd, int64_t offset, int whence);

/* Kind of filesystem object reported by rx_plat_stat. */
enum {
    RX_PLAT_KIND_OTHER = 0,
    RX_PLAT_KIND_FILE  = 1,
    RX_PLAT_KIND_DIR   = 2
};

typedef struct rx_plat_stat {
    uint64_t size;      /* bytes, 0 for directories when unknown */
    uint32_t kind;      /* RX_PLAT_KIND_* */
    uint32_t reserved;
} rx_plat_stat_t;

/* Query a path. out is only written on success. */
int rx_plat_stat(const char* path, rx_plat_stat_t* out);

/* Filesystem mutation. NeolyxOS currently returns RX_PLAT_ENOSYS for all of
 * these because its syscall table has no directory or unlink calls. */
int rx_plat_mkdir(const char* path);
int rx_plat_unlink(const char* path);
int rx_plat_rmdir(const char* path);
int rx_plat_rename(const char* old_path, const char* new_path);

/* ============================================================================
 * Process and environment
 * ============================================================================ */

/* Copy the value of an environment variable into buf (NUL terminated).
 * RX_PLAT_ENOENT when unset, RX_PLAT_ERANGE when cap is too small,
 * RX_PLAT_ENOSYS where there is no environment. */
int rx_plat_getenv(const char* name, char* buf, size_t cap);

/* Set an environment variable for this process. */
int rx_plat_setenv(const char* name, const char* value);

/* Run a shell command and return its exit status (>= 0) or a negative code. */
int rx_plat_exec(const char* command);

/* Working directory. */
int rx_plat_getcwd(char* buf, size_t cap);
int rx_plat_chdir(const char* path);

/* Process id, or a negative code. */
int64_t rx_plat_getpid(void);

/* ============================================================================
 * Time and lifecycle
 * ============================================================================ */

/* Monotonic milliseconds since an unspecified epoch (boot on NeolyxOS). */
uint64_t rx_plat_time_ms(void);

/* Seconds since 1970-01-01T00:00:00Z, or a negative code when the platform
 * has no real-time clock. */
int64_t rx_plat_time_unix(void);

/* Block the calling thread for at least ms milliseconds. */
int rx_plat_sleep_ms(uint64_t ms);

/* Terminate the process. Never returns. */
RX_PLAT_NORETURN void rx_plat_exit(int code);

/* Write msg (may be NULL) to the diagnostic channel and terminate abnormally.
 * Never returns. */
RX_PLAT_NORETURN void rx_plat_abort(const char* msg);

/* ============================================================================
 * Math shim
 *
 * Linux forwards to libm. NeolyxOS uses the freestanding implementations in
 * rx_math_neolyx.c, whose accuracy is documented at the top of that file.
 * ============================================================================ */

double rx_plat_sqrt(double x);
double rx_plat_cbrt(double x);
double rx_plat_sin(double x);
double rx_plat_cos(double x);
double rx_plat_tan(double x);
double rx_plat_asin(double x);
double rx_plat_acos(double x);
double rx_plat_atan(double x);
double rx_plat_atan2(double y, double x);
double rx_plat_exp(double x);
double rx_plat_log(double x);
double rx_plat_log2(double x);
double rx_plat_log10(double x);
double rx_plat_pow(double base, double exponent);
double rx_plat_floor(double x);
double rx_plat_ceil(double x);
double rx_plat_round(double x);
double rx_plat_trunc(double x);
double rx_plat_fabs(double x);
double rx_plat_fmod(double x, double y);

/* ============================================================================
 * Display and input shim
 *
 * The structures below mirror fb_info_t and input_event_t from
 * neolyx-os/desktop/include/nxsyscall.h field for field. The NeolyxOS
 * implementation copies between the two; if the kernel ABI changes, both
 * nxsyscall.h and these definitions must change together.
 * ============================================================================ */

typedef struct rx_plat_fb_info {
    void*    base;      /* mapped framebuffer address, NULL if unmapped */
    uint32_t width;     /* pixels */
    uint32_t height;    /* pixels */
    uint32_t pitch;     /* bytes per row */
    uint32_t bpp;       /* bits per pixel, normally 32 */
    uint32_t format;    /* 0 = BGRA, 1 = RGBA */
    uint32_t reserved;
    uint64_t size;      /* total bytes */
} rx_plat_fb_info_t;

#define RX_PLAT_INPUT_KEYBOARD      0u
#define RX_PLAT_INPUT_MOUSE_MOVE    1u
#define RX_PLAT_INPUT_MOUSE_BUTTON  2u

typedef struct rx_plat_input_event {
    uint32_t type;          /* RX_PLAT_INPUT_* */
    uint32_t timestamp;     /* milliseconds */
    union {
        struct {
            uint8_t scancode;
            uint8_t ascii;
            uint8_t pressed;    /* 1 pressed, 0 released */
            uint8_t modifiers;  /* shift, ctrl, alt bit flags */
        } keyboard;
        struct {
            int16_t dx;
            int16_t dy;
            uint8_t buttons;    /* button state bitmap */
            uint8_t pad[3];
        } mouse;
    } u;
} rx_plat_input_event_t;

/* Query the primary framebuffer. RX_PLAT_ENOSYS when there is no display. */
int rx_plat_fb_info(rx_plat_fb_info_t* out);

/* Map the framebuffer (or a backbuffer of the same geometry on hosts that
 * cannot expose the real one). NULL on failure. */
void* rx_plat_fb_map(void);

/* Present backbuffer (fb_info.size bytes) to the screen. */
int rx_plat_fb_flip(const void* backbuffer);

/* Fetch one input event. Returns 1 when ev was filled, 0 when no event is
 * pending, or a negative code. */
int rx_plat_input_poll(rx_plat_input_event_t* ev);

#ifdef __cplusplus
}
#endif

#endif /* RX_PLATFORM_H */
