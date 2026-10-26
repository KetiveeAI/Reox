/*
 * REOX Runtime - NeolyxOS platform implementation
 *
 * Implements platform/rx_platform.h on top of the NeolyxOS userspace syscall
 * layer (desktop/include/nxsyscall.h) and the nx_malloc heap
 * (desktop/lib/nx_malloc.c). No libc headers are included; the only
 * dependencies are the inline syscall wrappers and the two allocator symbols
 * that every NeolyxOS userspace binary already links.
 *
 * ABI notes (must stay in sync with the NeolyxOS kernel):
 *   - fb_info_t and input_event_t are copied field by field into the
 *     rx_plat_* mirrors declared in rx_platform.h.
 *   - Syscall numbers and flag values come from nxsyscall.h; rx_platform.h
 *     deliberately uses the same numeric values for open flags and seek
 *     origins so no translation table can drift.
 *   - Descriptors 1 and 2 are written with nx_write. If the kernel VFS has
 *     no console on those descriptors the write fails and the text is sent
 *     to the serial debug channel (SYS_DEBUG_PRINT) instead so diagnostics
 *     are never lost.
 *
 * Memory: nx_mfree is a no-op in the current nx_malloc, so freed memory is
 * not reclaimed on NeolyxOS. Each block carries a 16-byte header with its
 * size so rx_plat_realloc can copy and rx_plat_free can validate.
 */

#include "../rx_platform.h"
#include "nxsyscall.h"

/* ============================================================================
 * Memory
 * ============================================================================ */

#define RX_NX_MAGIC      0x52584d454d424c4bull   /* "RXMEMBLK" */
#define RX_NX_FREED      0x5258465245454421ull   /* "RXFREED!" */

typedef struct rx_nx_header {
    uint64_t magic;
    uint64_t size;
} rx_nx_header;   /* 16 bytes keeps the 16-byte alignment nx_malloc provides */

static rx_nx_header* rx_nx_header_of(void* ptr) {
    if (!ptr) return NULL;
    rx_nx_header* h = (rx_nx_header*)((unsigned char*)ptr - sizeof(rx_nx_header));
    if (h->magic != RX_NX_MAGIC) return NULL;   /* foreign or already freed */
    return h;
}

void* rx_plat_alloc(size_t size) {
    if (size == 0) return NULL;
    if (size > (size_t)UINT64_MAX - sizeof(rx_nx_header)) return NULL;
    void* raw = nx_malloc((uint64_t)size + sizeof(rx_nx_header));
    if (!raw) return NULL;
    rx_nx_header* h = (rx_nx_header*)raw;
    h->magic = RX_NX_MAGIC;
    h->size = (uint64_t)size;
    return (unsigned char*)raw + sizeof(rx_nx_header);
}

void rx_plat_free(void* ptr) {
    rx_nx_header* h = rx_nx_header_of(ptr);
    if (!h) return;             /* NULL, foreign pointer or double free: ignore */
    h->magic = RX_NX_FREED;
    nx_mfree(h);
}

void* rx_plat_realloc(void* ptr, size_t size) {
    if (!ptr) return rx_plat_alloc(size);
    if (size == 0) {
        rx_plat_free(ptr);
        return NULL;
    }
    rx_nx_header* h = rx_nx_header_of(ptr);
    if (!h) return NULL;        /* not one of ours: refuse rather than corrupt */
    if ((uint64_t)size <= h->size) {
        h->size = (uint64_t)size;   /* shrink in place */
        return ptr;
    }
    void* fresh = rx_plat_alloc(size);
    if (!fresh) return NULL;    /* original block stays valid */
    const unsigned char* s = (const unsigned char*)ptr;
    unsigned char* d = (unsigned char*)fresh;
    for (uint64_t i = 0; i < h->size; i++) d[i] = s[i];
    rx_plat_free(ptr);
    return fresh;
}

/* ============================================================================
 * Byte I/O
 * ============================================================================ */

static int rx_nx_err(int64_t rc) {
    /* The kernel returns small negative numbers; collapse them to EIO except
     * for the two the runtime acts on. */
    if (rc >= 0) return RX_PLAT_OK;
    if (rc == -2) return RX_PLAT_ENOENT;
    if (rc == -9) return RX_PLAT_EBADF;
    return RX_PLAT_EIO;
}

/* Serial fallback for console descriptors: nx_debug_print wants a NUL
 * terminated string, so copy through a small stack buffer. */
static void rx_nx_debug_write(const unsigned char* buf, size_t len) {
    char chunk[128];
    while (len > 0) {
        size_t n = len < sizeof(chunk) - 1 ? len : sizeof(chunk) - 1;
        for (size_t i = 0; i < n; i++) chunk[i] = (char)buf[i];
        chunk[n] = '\0';
        nx_debug_print(chunk);
        buf += n;
        len -= n;
    }
}

int64_t rx_plat_write(int fd, const void* buf, size_t len) {
    if (fd < 0) return RX_PLAT_EBADF;
    if (len == 0) return 0;
    if (!buf) return RX_PLAT_EINVAL;
    int64_t rc = nx_write(fd, buf, (uint64_t)len);
    if (rc >= 0) return rc;
    if (fd == RX_PLAT_STDOUT || fd == RX_PLAT_STDERR) {
        rx_nx_debug_write((const unsigned char*)buf, len);
        return (int64_t)len;
    }
    return rx_nx_err(rc);
}

int64_t rx_plat_read(int fd, void* buf, size_t len) {
    if (fd < 0) return RX_PLAT_EBADF;
    if (len == 0) return 0;
    if (!buf) return RX_PLAT_EINVAL;
    int64_t rc = nx_read(fd, buf, (uint64_t)len);
    if (rc < 0) return rx_nx_err(rc);
    return rc;
}

int rx_plat_open(const char* path, uint32_t flags) {
    if (!path) return RX_PLAT_EINVAL;
    /* rx_platform.h flag values equal the O_* values in nxsyscall.h */
    uint32_t known = RX_PLAT_O_RDONLY | RX_PLAT_O_WRONLY | RX_PLAT_O_RDWR |
                     RX_PLAT_O_CREAT | RX_PLAT_O_TRUNC | RX_PLAT_O_APPEND;
    if (flags & ~known) return RX_PLAT_EINVAL;
    int fd = nx_open(path, (int)flags);
    if (fd < 0) return rx_nx_err(fd);
    return fd;
}

int rx_plat_close(int fd) {
    if (fd < 0) return RX_PLAT_EBADF;
    return rx_nx_err(nx_close(fd));
}

int64_t rx_plat_seek(int fd, int64_t offset, int whence) {
    if (fd < 0) return RX_PLAT_EBADF;
    if (whence != RX_PLAT_SEEK_SET && whence != RX_PLAT_SEEK_CUR && whence != RX_PLAT_SEEK_END) {
        return RX_PLAT_EINVAL;
    }
    int64_t rc = nx_seek(fd, offset, whence);
    if (rc < 0) return rx_nx_err(rc);
    return rc;
}

/* Mirror of the kernel's stat_t for SYS_STAT. nxsyscall.h defines the
 * syscall number but no wrapper or struct, so the layout is declared here.
 * MUST MATCH kernel/include/core/syscall.h stat_t field for field. */
typedef struct rx_nx_stat {
    uint64_t st_ino;
    uint32_t st_mode;   /* 0040000 (octal) set for directories, 0100000 for files */
    uint32_t st_nlink;
    uint32_t st_uid;
    uint32_t st_gid;
    uint64_t st_size;
    uint64_t st_atime;
    uint64_t st_mtime;
    uint64_t st_ctime;
} rx_nx_stat_t;

#define RX_NX_MODE_DIR 0040000u   /* octal, as the kernel sets it */

int rx_plat_stat(const char* path, rx_plat_stat_t* out) {
    if (!path || !out) return RX_PLAT_EINVAL;
    rx_nx_stat_t st = { 0, 0, 0, 0, 0, 0, 0, 0, 0 };
    int64_t rc = syscall2(SYS_STAT, (int64_t)path, (int64_t)&st);
    if (rc < 0) return rx_nx_err(rc);
    out->size = st.st_size;
    out->kind = (st.st_mode & RX_NX_MODE_DIR) ? RX_PLAT_KIND_DIR : RX_PLAT_KIND_FILE;
    out->reserved = 0;
    return RX_PLAT_OK;
}

/* The NeolyxOS syscall table has no mkdir/unlink/rmdir/rename yet. */
int rx_plat_mkdir(const char* path) {
    if (!path) return RX_PLAT_EINVAL;
    return RX_PLAT_ENOSYS;
}

int rx_plat_unlink(const char* path) {
    if (!path) return RX_PLAT_EINVAL;
    return RX_PLAT_ENOSYS;
}

int rx_plat_rmdir(const char* path) {
    if (!path) return RX_PLAT_EINVAL;
    return RX_PLAT_ENOSYS;
}

int rx_plat_rename(const char* old_path, const char* new_path) {
    if (!old_path || !new_path) return RX_PLAT_EINVAL;
    return RX_PLAT_ENOSYS;
}

/* ============================================================================
 * Process and environment (no environment, no shell, no cwd on NeolyxOS)
 * ============================================================================ */

int rx_plat_getenv(const char* name, char* buf, size_t cap) {
    if (!name || !buf || cap == 0) return RX_PLAT_EINVAL;
    buf[0] = '\0';
    return RX_PLAT_ENOSYS;
}

int rx_plat_setenv(const char* name, const char* value) {
    if (!name || !value) return RX_PLAT_EINVAL;
    return RX_PLAT_ENOSYS;
}

int rx_plat_exec(const char* command) {
    if (!command) return RX_PLAT_EINVAL;
    return RX_PLAT_ENOSYS;
}

int rx_plat_getcwd(char* buf, size_t cap) {
    if (!buf || cap == 0) return RX_PLAT_EINVAL;
    /* Single-rooted VFS: every process runs at "/" */
    if (cap < 2) return RX_PLAT_ERANGE;
    buf[0] = '/';
    buf[1] = '\0';
    return RX_PLAT_OK;
}

int rx_plat_chdir(const char* path) {
    if (!path) return RX_PLAT_EINVAL;
    return RX_PLAT_ENOSYS;
}

int64_t rx_plat_getpid(void) {
    int64_t pid = syscall0(SYS_GETPID);
    if (pid < 0) return rx_nx_err(pid);
    return pid;
}

/* ============================================================================
 * Time and lifecycle
 * ============================================================================ */

uint64_t rx_plat_time_ms(void) {
    return nx_gettime();    /* milliseconds since boot */
}

int64_t rx_plat_time_unix(void) {
    uint64_t t = nx_rtc_unix();
    if (t == 0 || t > (uint64_t)INT64_MAX) return RX_PLAT_ENOSYS;   /* RTC absent */
    return (int64_t)t;
}

int rx_plat_sleep_ms(uint64_t ms) {
    if (ms == 0) {
        nx_yield();
        return RX_PLAT_OK;
    }
    int rc = nx_sleep(ms);
    return rc < 0 ? RX_PLAT_EIO : RX_PLAT_OK;
}

void rx_plat_exit(int code) {
    nx_exit(code);
    for (;;) { nx_yield(); }   /* nx_exit does not return; satisfy noreturn */
}

void rx_plat_abort(const char* msg) {
    if (msg) {
        nx_debug_print(msg);
        nx_debug_print("\n");
    } else {
        nx_debug_print("rx_plat_abort\n");
    }
    nx_exit(134);
    for (;;) { nx_yield(); }
}

/* ============================================================================
 * Display and input shim
 * ============================================================================ */

int rx_plat_fb_info(rx_plat_fb_info_t* out) {
    if (!out) return RX_PLAT_EINVAL;
    fb_info_t info;
    info.base_addr = 0; info.width = 0; info.height = 0; info.pitch = 0;
    info.bpp = 0; info.format = 0; info.size = 0;
    int rc = nx_fb_info(&info);
    if (rc < 0) return rx_nx_err(rc);
    if (info.width == 0 || info.height == 0 || info.pitch == 0) return RX_PLAT_EIO;
    out->base = (void*)(uintptr_t)info.base_addr;
    out->width = info.width;
    out->height = info.height;
    out->pitch = info.pitch;
    out->bpp = info.bpp;
    out->format = info.format;
    out->reserved = 0;
    out->size = info.size ? info.size : (uint64_t)info.pitch * info.height;
    return RX_PLAT_OK;
}

void* rx_plat_fb_map(void) {
    void* p = nx_fb_map();
    /* The syscall returns a negative error code cast to a pointer on failure */
    if ((intptr_t)p <= 0) return NULL;
    return p;
}

int rx_plat_fb_flip(const void* backbuffer) {
    if (!backbuffer) return RX_PLAT_EINVAL;
    int rc = nx_fb_flip((void*)backbuffer);
    return rc < 0 ? rx_nx_err(rc) : RX_PLAT_OK;
}

int rx_plat_input_poll(rx_plat_input_event_t* ev) {
    if (!ev) return RX_PLAT_EINVAL;
    input_event_t raw;
    raw.type = 0; raw.timestamp = 0;
    raw.mouse.dx = 0; raw.mouse.dy = 0; raw.mouse.buttons = 0;
    raw.mouse._pad[0] = raw.mouse._pad[1] = raw.mouse._pad[2] = 0;
    int rc = nx_input_poll(&raw);
    if (rc < 0) return rx_nx_err(rc);
    if (rc == 0) return 0;
    ev->type = raw.type;
    ev->timestamp = raw.timestamp;
    if (raw.type == INPUT_TYPE_KEYBOARD) {
        ev->u.keyboard.scancode = raw.keyboard.scancode;
        ev->u.keyboard.ascii = raw.keyboard.ascii;
        ev->u.keyboard.pressed = raw.keyboard.pressed;
        ev->u.keyboard.modifiers = raw.keyboard.modifiers;
    } else {
        ev->u.mouse.dx = raw.mouse.dx;
        ev->u.mouse.dy = raw.mouse.dy;
        ev->u.mouse.buttons = raw.mouse.buttons;
        ev->u.mouse.pad[0] = ev->u.mouse.pad[1] = ev->u.mouse.pad[2] = 0;
    }
    return 1;
}
