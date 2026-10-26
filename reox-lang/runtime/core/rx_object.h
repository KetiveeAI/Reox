/*
 * REOX Runtime - reference counted object header
 *
 * Every heap value the REOX compiler manages (strings, arrays, maps,
 * `ref struct` instances, layers, UI views) starts with an rx_object header.
 * The compiler emits rx_retain / rx_release around assignments, parameter
 * bindings, field stores and scope exits; when the count reaches zero the
 * object's dealloc hook runs exactly once and synchronously. There is no
 * collector, no finaliser queue and no deferred work: the moment the last
 * strong reference disappears is the moment the memory is released.
 *
 * Rules:
 *   - A refcount never wraps. Retaining an object whose count has reached
 *     RX_REFCOUNT_MAX is a hard abort through rx_plat_abort.
 *   - Releasing an object whose count is already zero is a hard abort.
 *   - Objects with refcount RX_REFCOUNT_IMMORTAL (string literals, static
 *     data) are never counted and never freed.
 *   - Weak references go through a zeroing table: rx_weak_load returns a
 *     strong (+1) pointer while the target is alive and NULL afterwards.
 *   - NULL is accepted everywhere and does nothing.
 *
 * Freestanding: only <stdint.h>, <stddef.h> and <stdbool.h>.
 */

#ifndef RX_OBJECT_H
#define RX_OBJECT_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct rx_object {
    uint32_t refcount;
    uint32_t type_id;
    void (*dealloc)(void*);
} rx_object;

/* Highest ordinary refcount. One more retain aborts. */
#define RX_REFCOUNT_MAX       0x7fffffffu
/* Marker for objects that live for the whole program. */
#define RX_REFCOUNT_IMMORTAL  0xffffffffu

/* Type ids of the runtime's own object kinds. Compiler generated types use
 * a 32-bit FNV-1a hash of their name with the top bit set. */
enum {
    RX_TYPE_NONE   = 0,
    RX_TYPE_STR    = 1,
    RX_TYPE_ARRAY  = 2,
    RX_TYPE_MAP    = 3,
    RX_TYPE_VIEW   = 4,
    RX_TYPE_WINDOW = 5,
    RX_TYPE_APP    = 6
};

/* Initialise a header in freshly allocated memory: refcount 1. */
void rx_object_init(rx_object* obj, uint32_t type_id, void (*dealloc)(void*));

/* Strong retain. Returns obj (NULL stays NULL). Aborts on overflow. */
void* rx_retain(void* obj);

/* Strong release. Runs dealloc when the count drops to zero. */
void rx_release(void* obj);

/* Current refcount, 0 for NULL. */
uint32_t rx_refcount(const void* obj);

/* Type id stored in the header, RX_TYPE_NONE for NULL. */
uint32_t rx_type_id(const void* obj);

/* Compile-time hash used by the compiler for user types; exposed so tests
 * and the runtime agree on the value. */
uint32_t rx_type_id_hash(const char* name);

/* Replace the strong reference in *slot with value, retaining the new value
 * first and releasing the old one afterwards (self assignment safe). */
void rx_assign(void** slot, void* value);

/* Release *slot and set it to NULL. */
void rx_clear(void** slot);

/* Store an already owned (+1) value into *slot, releasing the old value.
 * Unlike rx_assign the new value is not retained. */
void rx_store(void** slot, void* owned_value);

/* ============================================================================
 * Weak references
 *
 * rx_weak is a handle into a table of (target, generation) entries. Every
 * rx_weak_ref call takes one weak count on the entry; rx_weak_drop gives it
 * back. When the target is deallocated the entry's target is set to NULL
 * (zeroed) and any later rx_weak_load returns NULL. Entries are reused once
 * both the target is gone and the weak count is zero, with the generation
 * incremented so stale handles cannot alias a new object.
 * ============================================================================ */

typedef struct rx_weak {
    uint32_t index;
    uint32_t generation;
} rx_weak;

#define RX_WEAK_NONE_INDEX 0xffffffffu

/* A weak handle that refers to nothing. */
rx_weak rx_weak_none(void);

/* Register a weak reference to obj. NULL -> rx_weak_none(). Aborts when the
 * table cannot grow (out of memory) because the program would otherwise
 * silently lose the reference. */
rx_weak rx_weak_ref(void* obj);

/* Take another weak count on the same entry. Returns w. */
rx_weak rx_weak_copy(rx_weak w);

/* Give a weak count back. Safe on rx_weak_none(). */
void rx_weak_drop(rx_weak w);

/* Strong (+1) pointer to the target, or NULL if it was deallocated. */
void* rx_weak_load(rx_weak w);

/* Replace the weak handle in *slot: registers value, drops the old handle. */
void rx_weak_assign(rx_weak* slot, void* value);

/* Whether the weak handle still points at a live object. */
bool rx_weak_alive(rx_weak w);

/* Number of live entries in the weak table (for tests and diagnostics). */
size_t rx_weak_table_count(void);

/* Release the weak table's storage. Only valid when no entries are in use;
 * called at process exit by tests so leak checkers see a clean heap. */
void rx_weak_table_shutdown(void);

#ifdef __cplusplus
}
#endif

#endif /* RX_OBJECT_H */
