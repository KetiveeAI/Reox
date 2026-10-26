/*
 * REOX Runtime - Linux platform implementation
 *
 * Implements platform/rx_platform.h with glibc, libm and (optionally) SDL2
 * for the framebuffer/input shim. This file is the only place in librx where
 * POSIX headers are allowed besides the SDL backend sources beside it.
 *
 * Build with -DRX_HAVE_SDL (the Makefile adds it when pkg-config finds sdl2)
 * to get a window-backed framebuffer; without it the display shim returns
 * RX_PLAT_ENOSYS and everything else still works headless.
 */

#define _GNU_SOURCE
#include "../rx_platform.h"

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <errno.h>
#include <math.h>
#include <time.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <sys/types.h>

#ifdef RX_HAVE_SDL
#include <SDL2/SDL.h>
#endif

/* ============================================================================
 * Error mapping
 * ============================================================================ */

static int rx_linux_errno(int e) {
    switch (e) {
        case 0:       return RX_PLAT_OK;
        case ENOENT:  return RX_PLAT_ENOENT;
        case EEXIST:  return RX_PLAT_EEXIST;
        case ENOMEM:  return RX_PLAT_ENOMEM;
        case EBADF:   return RX_PLAT_EBADF;
        case EINVAL:  return RX_PLAT_EINVAL;
        case ERANGE:  return RX_PLAT_ERANGE;
        case ENOSYS:  return RX_PLAT_ENOSYS;
        default:      return RX_PLAT_EIO;
    }
}

/* ============================================================================
 * Memory
 * ============================================================================ */

void* rx_plat_alloc(size_t size) {
    if (size == 0) return NULL;
    void* p = NULL;
    /* 16-byte alignment matches the NeolyxOS allocator */
    if (posix_memalign(&p, 16, size) != 0) return NULL;
    return p;
}

void* rx_plat_realloc(void* ptr, size_t size) {
    if (size == 0) {
        free(ptr);
        return NULL;
    }
    if (!ptr) return rx_plat_alloc(size);
    return realloc(ptr, size);
}

void rx_plat_free(void* ptr) {
    free(ptr);
}

/* ============================================================================
 * Byte I/O
 * ============================================================================ */

int64_t rx_plat_write(int fd, const void* buf, size_t len) {
    if (fd < 0) return RX_PLAT_EBADF;
    if (len == 0) return 0;
    if (!buf) return RX_PLAT_EINVAL;
    for (;;) {
        ssize_t n = write(fd, buf, len);
        if (n >= 0) return (int64_t)n;
        if (errno == EINTR) continue;
        return rx_linux_errno(errno);
    }
}

int64_t rx_plat_read(int fd, void* buf, size_t len) {
    if (fd < 0) return RX_PLAT_EBADF;
    if (len == 0) return 0;
    if (!buf) return RX_PLAT_EINVAL;
    for (;;) {
        ssize_t n = read(fd, buf, len);
        if (n >= 0) return (int64_t)n;
        if (errno == EINTR) continue;
        return rx_linux_errno(errno);
    }
}

int rx_plat_open(const char* path, uint32_t flags) {
    if (!path) return RX_PLAT_EINVAL;
    int oflags = 0;
    switch (flags & 0x3u) {
        case RX_PLAT_O_RDONLY: oflags = O_RDONLY; break;
        case RX_PLAT_O_WRONLY: oflags = O_WRONLY; break;
        case RX_PLAT_O_RDWR:   oflags = O_RDWR;   break;
        default: return RX_PLAT_EINVAL;
    }
    if (flags & RX_PLAT_O_CREAT)  oflags |= O_CREAT;
    if (flags & RX_PLAT_O_TRUNC)  oflags |= O_TRUNC;
    if (flags & RX_PLAT_O_APPEND) oflags |= O_APPEND;
    oflags |= O_CLOEXEC;
    int fd = open(path, oflags, 0644);
    if (fd < 0) return rx_linux_errno(errno);
    return fd;
}

int rx_plat_close(int fd) {
    if (fd < 0) return RX_PLAT_EBADF;
    if (close(fd) != 0) return rx_linux_errno(errno);
    return RX_PLAT_OK;
}

int64_t rx_plat_seek(int fd, int64_t offset, int whence) {
    if (fd < 0) return RX_PLAT_EBADF;
    int w;
    switch (whence) {
        case RX_PLAT_SEEK_SET: w = SEEK_SET; break;
        case RX_PLAT_SEEK_CUR: w = SEEK_CUR; break;
        case RX_PLAT_SEEK_END: w = SEEK_END; break;
        default: return RX_PLAT_EINVAL;
    }
    off_t r = lseek(fd, (off_t)offset, w);
    if (r < 0) return rx_linux_errno(errno);
    return (int64_t)r;
}

int rx_plat_stat(const char* path, rx_plat_stat_t* out) {
    if (!path || !out) return RX_PLAT_EINVAL;
    struct stat st;
    if (stat(path, &st) != 0) return rx_linux_errno(errno);
    out->size = (uint64_t)st.st_size;
    out->kind = S_ISDIR(st.st_mode) ? RX_PLAT_KIND_DIR :
                S_ISREG(st.st_mode) ? RX_PLAT_KIND_FILE : RX_PLAT_KIND_OTHER;
    out->reserved = 0;
    return RX_PLAT_OK;
}

int rx_plat_mkdir(const char* path) {
    if (!path) return RX_PLAT_EINVAL;
    if (mkdir(path, 0755) != 0) return rx_linux_errno(errno);
    return RX_PLAT_OK;
}

int rx_plat_unlink(const char* path) {
    if (!path) return RX_PLAT_EINVAL;
    if (unlink(path) != 0) return rx_linux_errno(errno);
    return RX_PLAT_OK;
}

int rx_plat_rmdir(const char* path) {
    if (!path) return RX_PLAT_EINVAL;
    if (rmdir(path) != 0) return rx_linux_errno(errno);
    return RX_PLAT_OK;
}

int rx_plat_rename(const char* old_path, const char* new_path) {
    if (!old_path || !new_path) return RX_PLAT_EINVAL;
    if (rename(old_path, new_path) != 0) return rx_linux_errno(errno);
    return RX_PLAT_OK;
}

/* ============================================================================
 * Process and environment
 * ============================================================================ */

int rx_plat_getenv(const char* name, char* buf, size_t cap) {
    if (!name || !buf || cap == 0) return RX_PLAT_EINVAL;
    const char* v = getenv(name);
    if (!v) return RX_PLAT_ENOENT;
    size_t len = strlen(v);
    if (len + 1 > cap) return RX_PLAT_ERANGE;
    memcpy(buf, v, len + 1);
    return RX_PLAT_OK;
}

int rx_plat_setenv(const char* name, const char* value) {
    if (!name || !value) return RX_PLAT_EINVAL;
    if (setenv(name, value, 1) != 0) return rx_linux_errno(errno);
    return RX_PLAT_OK;
}

int rx_plat_exec(const char* command) {
    if (!command) return RX_PLAT_EINVAL;
    int st = system(command);
    if (st < 0) return rx_linux_errno(errno);
    return WIFEXITED(st) ? WEXITSTATUS(st) : RX_PLAT_EIO;
}

int rx_plat_getcwd(char* buf, size_t cap) {
    if (!buf || cap == 0) return RX_PLAT_EINVAL;
    if (!getcwd(buf, cap)) return rx_linux_errno(errno);
    return RX_PLAT_OK;
}

int rx_plat_chdir(const char* path) {
    if (!path) return RX_PLAT_EINVAL;
    if (chdir(path) != 0) return rx_linux_errno(errno);
    return RX_PLAT_OK;
}

int64_t rx_plat_getpid(void) {
    return (int64_t)getpid();
}

/* ============================================================================
 * Time and lifecycle
 * ============================================================================ */

uint64_t rx_plat_time_ms(void) {
    struct timespec ts;
    if (clock_gettime(CLOCK_MONOTONIC, &ts) != 0) return 0;
    return (uint64_t)ts.tv_sec * 1000ull + (uint64_t)ts.tv_nsec / 1000000ull;
}

int64_t rx_plat_time_unix(void) {
    struct timespec ts;
    if (clock_gettime(CLOCK_REALTIME, &ts) != 0) return RX_PLAT_EIO;
    return (int64_t)ts.tv_sec;
}

int rx_plat_sleep_ms(uint64_t ms) {
    struct timespec ts;
    ts.tv_sec = (time_t)(ms / 1000ull);
    ts.tv_nsec = (long)((ms % 1000ull) * 1000000ull);
    while (nanosleep(&ts, &ts) != 0) {
        if (errno != EINTR) return rx_linux_errno(errno);
    }
    return RX_PLAT_OK;
}

void rx_plat_exit(int code) {
    fflush(NULL);
    exit(code);
}

void rx_plat_abort(const char* msg) {
    if (msg) {
        size_t len = strlen(msg);
        ssize_t w = write(STDERR_FILENO, msg, len);
        (void)w;
        w = write(STDERR_FILENO, "\n", 1);
        (void)w;
    }
    abort();
}

/* ============================================================================
 * Math shim (libm)
 * ============================================================================ */

double rx_plat_sqrt(double x)             { return sqrt(x); }
double rx_plat_cbrt(double x)             { return cbrt(x); }
double rx_plat_sin(double x)              { return sin(x); }
double rx_plat_cos(double x)              { return cos(x); }
double rx_plat_tan(double x)              { return tan(x); }
double rx_plat_asin(double x)             { return asin(x); }
double rx_plat_acos(double x)             { return acos(x); }
double rx_plat_atan(double x)             { return atan(x); }
double rx_plat_atan2(double y, double x)  { return atan2(y, x); }
double rx_plat_exp(double x)              { return exp(x); }
double rx_plat_log(double x)              { return log(x); }
double rx_plat_log2(double x)             { return log2(x); }
double rx_plat_log10(double x)            { return log10(x); }
double rx_plat_pow(double b, double e)    { return pow(b, e); }
double rx_plat_floor(double x)            { return floor(x); }
double rx_plat_ceil(double x)             { return ceil(x); }
double rx_plat_round(double x)            { return round(x); }
double rx_plat_trunc(double x)            { return trunc(x); }
double rx_plat_fabs(double x)             { return fabs(x); }
double rx_plat_fmod(double x, double y)   { return fmod(x, y); }

/* ============================================================================
 * Display and input shim
 * ============================================================================ */

#ifdef RX_HAVE_SDL

#define RX_FB_WIDTH   1024u
#define RX_FB_HEIGHT  768u

static SDL_Window*   g_fb_window = NULL;
static SDL_Renderer* g_fb_renderer = NULL;
static SDL_Texture*  g_fb_texture = NULL;
static uint32_t*     g_fb_backbuffer = NULL;

static int rx_fb_ensure(void) {
    if (g_fb_window) return RX_PLAT_OK;
    if (SDL_WasInit(SDL_INIT_VIDEO) == 0 && SDL_Init(SDL_INIT_VIDEO) != 0) return RX_PLAT_EIO;
    g_fb_window = SDL_CreateWindow("REOX framebuffer", SDL_WINDOWPOS_CENTERED, SDL_WINDOWPOS_CENTERED,
                                   (int)RX_FB_WIDTH, (int)RX_FB_HEIGHT, SDL_WINDOW_SHOWN);
    if (!g_fb_window) return RX_PLAT_EIO;
    g_fb_renderer = SDL_CreateRenderer(g_fb_window, -1, SDL_RENDERER_ACCELERATED | SDL_RENDERER_PRESENTVSYNC);
    if (!g_fb_renderer) {
        SDL_DestroyWindow(g_fb_window); g_fb_window = NULL;
        return RX_PLAT_EIO;
    }
    g_fb_texture = SDL_CreateTexture(g_fb_renderer, SDL_PIXELFORMAT_ARGB8888, SDL_TEXTUREACCESS_STREAMING,
                                     (int)RX_FB_WIDTH, (int)RX_FB_HEIGHT);
    g_fb_backbuffer = (uint32_t*)calloc((size_t)RX_FB_WIDTH * RX_FB_HEIGHT, sizeof(uint32_t));
    if (!g_fb_texture || !g_fb_backbuffer) {
        if (g_fb_texture) SDL_DestroyTexture(g_fb_texture);
        free(g_fb_backbuffer);
        SDL_DestroyRenderer(g_fb_renderer);
        SDL_DestroyWindow(g_fb_window);
        g_fb_texture = NULL; g_fb_backbuffer = NULL; g_fb_renderer = NULL; g_fb_window = NULL;
        return RX_PLAT_ENOMEM;
    }
    return RX_PLAT_OK;
}

int rx_plat_fb_info(rx_plat_fb_info_t* out) {
    if (!out) return RX_PLAT_EINVAL;
    int rc = rx_fb_ensure();
    if (rc != RX_PLAT_OK) return rc;
    out->base = g_fb_backbuffer;
    out->width = RX_FB_WIDTH;
    out->height = RX_FB_HEIGHT;
    out->pitch = RX_FB_WIDTH * 4u;
    out->bpp = 32;
    out->format = 0;    /* BGRA in memory == ARGB8888 little endian */
    out->reserved = 0;
    out->size = (uint64_t)RX_FB_WIDTH * RX_FB_HEIGHT * 4u;
    return RX_PLAT_OK;
}

void* rx_plat_fb_map(void) {
    if (rx_fb_ensure() != RX_PLAT_OK) return NULL;
    return g_fb_backbuffer;
}

int rx_plat_fb_flip(const void* backbuffer) {
    if (!backbuffer) return RX_PLAT_EINVAL;
    int rc = rx_fb_ensure();
    if (rc != RX_PLAT_OK) return rc;
    if (SDL_UpdateTexture(g_fb_texture, NULL, backbuffer, (int)(RX_FB_WIDTH * 4u)) != 0) return RX_PLAT_EIO;
    if (SDL_RenderClear(g_fb_renderer) != 0) return RX_PLAT_EIO;
    if (SDL_RenderCopy(g_fb_renderer, g_fb_texture, NULL, NULL) != 0) return RX_PLAT_EIO;
    SDL_RenderPresent(g_fb_renderer);
    return RX_PLAT_OK;
}

int rx_plat_input_poll(rx_plat_input_event_t* ev) {
    if (!ev) return RX_PLAT_EINVAL;
    int rc = rx_fb_ensure();
    if (rc != RX_PLAT_OK) return rc;
    SDL_Event e;
    while (SDL_PollEvent(&e)) {
        memset(ev, 0, sizeof(*ev));
        ev->timestamp = (uint32_t)e.common.timestamp;
        switch (e.type) {
            case SDL_QUIT:
                /* Report as Escape so a freestanding event loop can exit */
                ev->type = RX_PLAT_INPUT_KEYBOARD;
                ev->u.keyboard.scancode = 0x01;
                ev->u.keyboard.ascii = 27;
                ev->u.keyboard.pressed = 1;
                return 1;
            case SDL_KEYDOWN:
            case SDL_KEYUP: {
                ev->type = RX_PLAT_INPUT_KEYBOARD;
                ev->u.keyboard.scancode = (uint8_t)(e.key.keysym.scancode & 0xFF);
                SDL_Keycode kc = e.key.keysym.sym;
                ev->u.keyboard.ascii = (kc >= 0 && kc < 128) ? (uint8_t)kc : 0;
                ev->u.keyboard.pressed = (e.type == SDL_KEYDOWN) ? 1 : 0;
                uint8_t mods = 0;
                if (e.key.keysym.mod & KMOD_SHIFT) mods |= 1u;
                if (e.key.keysym.mod & KMOD_CTRL)  mods |= 2u;
                if (e.key.keysym.mod & KMOD_ALT)   mods |= 4u;
                ev->u.keyboard.modifiers = mods;
                return 1;
            }
            case SDL_MOUSEMOTION:
                ev->type = RX_PLAT_INPUT_MOUSE_MOVE;
                ev->u.mouse.dx = (int16_t)e.motion.xrel;
                ev->u.mouse.dy = (int16_t)e.motion.yrel;
                ev->u.mouse.buttons = (uint8_t)(e.motion.state & 0xFF);
                return 1;
            case SDL_MOUSEBUTTONDOWN:
            case SDL_MOUSEBUTTONUP:
                ev->type = RX_PLAT_INPUT_MOUSE_BUTTON;
                ev->u.mouse.buttons = (uint8_t)(SDL_GetMouseState(NULL, NULL) & 0xFF);
                return 1;
            default:
                continue;   /* ignore window/system events */
        }
    }
    return 0;
}

#else /* !RX_HAVE_SDL: headless host */

int rx_plat_fb_info(rx_plat_fb_info_t* out) {
    if (!out) return RX_PLAT_EINVAL;
    return RX_PLAT_ENOSYS;
}

void* rx_plat_fb_map(void) {
    return NULL;
}

int rx_plat_fb_flip(const void* backbuffer) {
    if (!backbuffer) return RX_PLAT_EINVAL;
    return RX_PLAT_ENOSYS;
}

int rx_plat_input_poll(rx_plat_input_event_t* ev) {
    if (!ev) return RX_PLAT_EINVAL;
    return RX_PLAT_ENOSYS;
}

#endif /* RX_HAVE_SDL */
