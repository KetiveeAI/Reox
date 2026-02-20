/*
 * REOX System Stdlib Wrappers
 * Maps system.rx stdlib declarations to POSIX/NeolyxOS system calls
 * 
 * Copyright (c) 2025-2026 KetiveeAI
 */

#ifndef REOX_SYSTEM_STDLIB_H
#define REOX_SYSTEM_STDLIB_H

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>
#include <dirent.h>
#include <errno.h>
#include <time.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ============================================================================
 * File Operations (from system.rx)
 * ============================================================================ */

/* file_read(path: string) -> string */
static inline const char* file_read(const char* path) {
    if (!path) return NULL;
    
    FILE* f = fopen(path, "rb");
    if (!f) return NULL;
    
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    
    char* buffer = (char*)malloc(size + 1);
    if (!buffer) {
        fclose(f);
        return NULL;
    }
    
    fread(buffer, 1, size, f);
    buffer[size] = '\0';
    fclose(f);
    
    return buffer;
}

/* file_write(path: string, content: string) -> bool */
static inline int64_t file_write(const char* path, const char* content) {
    if (!path || !content) return 0;
    
    FILE* f = fopen(path, "wb");
    if (!f) return 0;
    
    size_t len = strlen(content);
    size_t written = fwrite(content, 1, len, f);
    fclose(f);
    
    return written == len ? 1 : 0;
}

/* file_append(path: string, content: string) -> bool */
static inline int64_t file_append(const char* path, const char* content) {
    if (!path || !content) return 0;
    
    FILE* f = fopen(path, "ab");
    if (!f) return 0;
    
    size_t len = strlen(content);
    size_t written = fwrite(content, 1, len, f);
    fclose(f);
    
    return written == len ? 1 : 0;
}

/* file_exists(path: string) -> bool */
static inline int64_t file_exists(const char* path) {
    if (!path) return 0;
    return access(path, F_OK) == 0 ? 1 : 0;
}

/* file_delete(path: string) -> bool */
static inline int64_t file_delete(const char* path) {
    if (!path) return 0;
    return remove(path) == 0 ? 1 : 0;
}

/* file_size(path: string) -> int */
static inline int64_t file_size(const char* path) {
    if (!path) return -1;
    
    struct stat st;
    if (stat(path, &st) != 0) return -1;
    
    return (int64_t)st.st_size;
}

/* file_copy(src: string, dest: string) -> bool */
static inline int64_t file_copy(const char* src, const char* dest) {
    if (!src || !dest) return 0;
    
    FILE* fsrc = fopen(src, "rb");
    if (!fsrc) return 0;
    
    FILE* fdest = fopen(dest, "wb");
    if (!fdest) {
        fclose(fsrc);
        return 0;
    }
    
    char buffer[8192];
    size_t bytes;
    while ((bytes = fread(buffer, 1, sizeof(buffer), fsrc)) > 0) {
        fwrite(buffer, 1, bytes, fdest);
    }
    
    fclose(fsrc);
    fclose(fdest);
    return 1;
}

/* file_rename(old_path: string, new_path: string) -> bool */
static inline int64_t file_rename(const char* old_path, const char* new_path) {
    if (!old_path || !new_path) return 0;
    return rename(old_path, new_path) == 0 ? 1 : 0;
}

/* ============================================================================
 * Directory Operations
 * ============================================================================ */

/* dir_create(path: string) -> bool */
static inline int64_t dir_create(const char* path) {
    if (!path) return 0;
    return mkdir(path, 0755) == 0 ? 1 : 0;
}

/* dir_exists(path: string) -> bool */
static inline int64_t dir_exists(const char* path) {
    if (!path) return 0;
    
    struct stat st;
    if (stat(path, &st) != 0) return 0;
    
    return S_ISDIR(st.st_mode) ? 1 : 0;
}

/* dir_delete(path: string) -> bool */
static inline int64_t dir_delete(const char* path) {
    if (!path) return 0;
    return rmdir(path) == 0 ? 1 : 0;
}

/* ============================================================================
 * Environment Variables
 * ============================================================================ */

/* env_get(name: string) -> string */
static inline const char* env_get(const char* name) {
    if (!name) return NULL;
    return getenv(name);
}

/* env_set(name: string, value: string) -> bool */
static inline int64_t env_set(const char* name, const char* value) {
    if (!name || !value) return 0;
    return setenv(name, value, 1) == 0 ? 1 : 0;
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
    rx_sys_args.argc = argc;
    rx_sys_args.argv = argv;
}

/* sys_args() -> Array */
static inline int64_t sys_args_count(void) {
    return rx_sys_args.argc;
}

static inline const char* sys_args_get(int64_t index) {
    if (index < 0 || index >= rx_sys_args.argc) return NULL;
    return rx_sys_args.argv[index];
}

/* sys_exit(code: int) -> void */
static inline void sys_exit_rx(int64_t code) {
    exit((int)code);
}

/* sys_exec(command: string) -> int */
static inline int64_t sys_exec(const char* command) {
    if (!command) return -1;
    return (int64_t)system(command);
}

/* sys_pid() -> int */
static inline int64_t sys_pid(void) {
    return (int64_t)getpid();
}

/* sys_sleep(ms: int) -> void */
static inline void sys_sleep(int64_t ms) {
    usleep((useconds_t)(ms * 1000));
}

/* ============================================================================
 * Time Functions
 * ============================================================================ */

/* time_now() -> int (Unix timestamp in seconds) */
static inline int64_t time_now(void) {
    return (int64_t)time(NULL);
}

/* time_ms() -> int (Milliseconds since epoch) */
static inline int64_t time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    return (int64_t)(ts.tv_sec * 1000 + ts.tv_nsec / 1000000);
}

/* time_format(timestamp: int, format: string) -> string */
static inline const char* time_format(int64_t timestamp, const char* format) {
    static char buffer[128];
    time_t t = (time_t)timestamp;
    struct tm* tm_info = localtime(&t);
    
    if (!format) format = "%Y-%m-%d %H:%M:%S";
    strftime(buffer, sizeof(buffer), format, tm_info);
    
    return buffer;
}

/* ============================================================================
 * Path Utilities
 * ============================================================================ */

/* path_join(a: string, b: string) -> string */
static inline const char* path_join(const char* a, const char* b) {
    if (!a || !b) return NULL;
    
    size_t len_a = strlen(a);
    size_t len_b = strlen(b);
    
    char* result = (char*)malloc(len_a + len_b + 2);
    if (!result) return NULL;
    
    strcpy(result, a);
    if (len_a > 0 && a[len_a - 1] != '/') {
        strcat(result, "/");
    }
    strcat(result, b);
    
    return result;
}

/* path_basename(path: string) -> string */
static inline const char* path_basename(const char* path) {
    if (!path) return NULL;
    
    const char* last = strrchr(path, '/');
    return last ? last + 1 : path;
}

/* path_dirname(path: string) -> string */
static inline const char* path_dirname(const char* path) {
    if (!path) return NULL;
    
    const char* last = strrchr(path, '/');
    if (!last) return ".";
    
    size_t len = last - path;
    char* result = (char*)malloc(len + 1);
    if (!result) return NULL;
    
    strncpy(result, path, len);
    result[len] = '\0';
    
    return result;
}

/* path_extension(path: string) -> string */
static inline const char* path_extension(const char* path) {
    if (!path) return NULL;
    
    const char* basename = path_basename(path);
    const char* dot = strrchr(basename, '.');
    
    return dot ? dot + 1 : "";
}

/* cwd() -> string */
static inline const char* cwd(void) {
    static char buffer[4096];
    return getcwd(buffer, sizeof(buffer));
}

/* chdir_rx(path: string) -> bool */
static inline int64_t chdir_rx(const char* path) {
    if (!path) return 0;
    return chdir(path) == 0 ? 1 : 0;
}

#ifdef __cplusplus
}
#endif

#endif /* REOX_SYSTEM_STDLIB_H */
