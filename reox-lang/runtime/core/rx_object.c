/*
 * REOX Runtime - reference counting and the zeroing weak table
 *
 * See rx_object.h for the contract. Everything here is deterministic: the
 * same sequence of retain/release/weak calls always performs the same
 * allocations and frees in the same order.
 */

#include "rx_core.h"

/* ============================================================================
 * Strong references
 * ============================================================================ */

void rx_object_init(rx_object* obj, uint32_t type_id, void (*dealloc)(void*)) {
    if (!obj) return;
    obj->refcount = 1;
    obj->type_id = type_id;
    obj->dealloc = dealloc;
}

void* rx_retain(void* obj) {
    rx_object* o = (rx_object*)obj;
    if (!o) return NULL;
    if (o->refcount == RX_REFCOUNT_IMMORTAL) return obj;
    if (RX_UNLIKELY(o->refcount >= RX_REFCOUNT_MAX)) {
        rx_plat_abort("rx_retain: reference count overflow");
    }
    if (RX_UNLIKELY(o->refcount == 0)) {
        rx_plat_abort("rx_retain: object is already deallocated");
    }
    o->refcount++;
    return obj;
}

static void rx_weak_table_zero(const void* obj);

void rx_release(void* obj) {
    rx_object* o = (rx_object*)obj;
    if (!o) return;
    if (o->refcount == RX_REFCOUNT_IMMORTAL) return;
    if (RX_UNLIKELY(o->refcount == 0)) {
        rx_plat_abort("rx_release: object is already deallocated");
    }
    o->refcount--;
    if (o->refcount == 0) {
        rx_weak_table_zero(o);
        if (o->dealloc) o->dealloc(obj);
    }
}

uint32_t rx_refcount(const void* obj) {
    return obj ? ((const rx_object*)obj)->refcount : 0;
}

uint32_t rx_type_id(const void* obj) {
    return obj ? ((const rx_object*)obj)->type_id : RX_TYPE_NONE;
}

uint32_t rx_type_id_hash(const char* name) {
    uint32_t h = 2166136261u;
    if (name) {
        while (*name) {
            h ^= (unsigned char)*name++;
            h *= 16777619u;
        }
    }
    return h | 0x80000000u;
}

void rx_assign(void** slot, void* value) {
    if (!slot) return;
    void* old = *slot;
    rx_retain(value);
    *slot = value;
    rx_release(old);
}

void rx_clear(void** slot) {
    if (!slot) return;
    void* old = *slot;
    *slot = NULL;
    rx_release(old);
}

/* ============================================================================
 * Weak table
 *
 * entries[] is a dense array indexed by rx_weak.index. A hash index keyed by
 * target pointer (open addressing, linear probing) lets rx_release find the
 * entry for a dying object without scanning. Free entries form a singly
 * linked list through next_free.
 * ============================================================================ */

typedef struct rx_weak_entry {
    void*    target;        /* NULL once the object is gone */
    uint32_t generation;    /* bumped every time the entry is recycled */
    uint32_t weak_count;    /* outstanding handles */
    uint32_t next_free;     /* free list link, RX_WEAK_NONE_INDEX at end */
    uint32_t in_use;        /* 1 while the entry holds a handle or a target */
} rx_weak_entry;

static rx_weak_entry* g_entries = NULL;
static uint32_t g_entry_count = 0;       /* entries allocated in g_entries */
static uint32_t g_entry_capacity = 0;
static uint32_t g_free_head = RX_WEAK_NONE_INDEX;
static size_t   g_live = 0;              /* in_use entries */

/* Hash index: slot holds entry index + 1, 0 means empty. */
static uint32_t* g_index = NULL;
static uint32_t g_index_capacity = 0;    /* power of two */
static uint32_t g_index_used = 0;

static uint32_t rx_ptr_hash(const void* p) {
    uintptr_t v = (uintptr_t)p;
    v ^= v >> 17;
    v *= (uintptr_t)0x9E3779B97F4A7C15ull;
    v ^= v >> 29;
    return (uint32_t)v;
}

static bool rx_index_insert_raw(uint32_t* table, uint32_t cap, const void* target, uint32_t entry) {
    uint32_t mask = cap - 1;
    uint32_t i = rx_ptr_hash(target) & mask;
    for (uint32_t n = 0; n < cap; n++) {
        if (table[i] == 0) {
            table[i] = entry + 1;
            return true;
        }
        i = (i + 1) & mask;
    }
    return false;
}

static bool rx_index_grow(void) {
    uint32_t new_cap = g_index_capacity ? g_index_capacity * 2 : 16;
    if (new_cap == 0) return false;                         /* 2^32 entries: wrapped */
    uint32_t* table = (uint32_t*)rx_plat_alloc((size_t)new_cap * sizeof(uint32_t));
    if (!table) return false;
    rx_memset(table, 0, (size_t)new_cap * sizeof(uint32_t));
    for (uint32_t e = 0; e < g_entry_count; e++) {
        if (g_entries[e].in_use && g_entries[e].target) {
            (void)rx_index_insert_raw(table, new_cap, g_entries[e].target, e);
        }
    }
    rx_plat_free(g_index);
    g_index = table;
    g_index_capacity = new_cap;
    return true;
}

static bool rx_index_insert(const void* target, uint32_t entry) {
    if ((g_index_used + 1) * 2 > g_index_capacity) {
        if (!rx_index_grow()) return false;
    }
    if (!rx_index_insert_raw(g_index, g_index_capacity, target, entry)) return false;
    g_index_used++;
    return true;
}

/* Find the index slot holding target's entry, or RX_WEAK_NONE_INDEX. */
static uint32_t rx_index_find_slot(const void* target) {
    if (g_index_capacity == 0) return RX_WEAK_NONE_INDEX;
    uint32_t mask = g_index_capacity - 1;
    uint32_t i = rx_ptr_hash(target) & mask;
    for (uint32_t n = 0; n < g_index_capacity; n++) {
        uint32_t v = g_index[i];
        if (v == 0) return RX_WEAK_NONE_INDEX;
        if (g_entries[v - 1].target == target) return i;
        i = (i + 1) & mask;
    }
    return RX_WEAK_NONE_INDEX;
}

/* Remove a slot from the open addressing index, re-inserting the cluster
 * that follows it so lookups stay correct. */
static void rx_index_remove_slot(uint32_t slot) {
    uint32_t mask = g_index_capacity - 1;
    g_index[slot] = 0;
    g_index_used--;
    uint32_t i = (slot + 1) & mask;
    while (g_index[i] != 0) {
        uint32_t entry = g_index[i] - 1;
        g_index[i] = 0;
        (void)rx_index_insert_raw(g_index, g_index_capacity, g_entries[entry].target, entry);
        i = (i + 1) & mask;
    }
}

static uint32_t rx_entry_alloc(void) {
    if (g_free_head != RX_WEAK_NONE_INDEX) {
        uint32_t e = g_free_head;
        g_free_head = g_entries[e].next_free;
        g_entries[e].next_free = RX_WEAK_NONE_INDEX;
        return e;
    }
    if (g_entry_count == g_entry_capacity) {
        uint32_t new_cap = g_entry_capacity ? g_entry_capacity * 2 : 16;
        if (new_cap == 0) return RX_WEAK_NONE_INDEX;        /* wrapped */
        rx_weak_entry* grown = (rx_weak_entry*)rx_plat_realloc(g_entries, (size_t)new_cap * sizeof(rx_weak_entry));
        if (!grown) return RX_WEAK_NONE_INDEX;
        g_entries = grown;
        g_entry_capacity = new_cap;
    }
    uint32_t e = g_entry_count++;
    g_entries[e].target = NULL;
    g_entries[e].generation = 1;
    g_entries[e].weak_count = 0;
    g_entries[e].next_free = RX_WEAK_NONE_INDEX;
    g_entries[e].in_use = 0;
    return e;
}

static void rx_entry_recycle(uint32_t e) {
    g_entries[e].in_use = 0;
    g_entries[e].target = NULL;
    g_entries[e].weak_count = 0;
    g_entries[e].generation++;
    if (g_entries[e].generation == 0) g_entries[e].generation = 1;
    g_entries[e].next_free = g_free_head;
    g_free_head = e;
    g_live--;
}

static bool rx_weak_valid(rx_weak w) {
    return w.index != RX_WEAK_NONE_INDEX
        && w.index < g_entry_count
        && g_entries[w.index].in_use
        && g_entries[w.index].generation == w.generation;
}

static void rx_weak_table_zero(const void* obj) {
    if (g_live == 0) return;
    uint32_t slot = rx_index_find_slot(obj);
    if (slot == RX_WEAK_NONE_INDEX) return;
    uint32_t e = g_index[slot] - 1;
    rx_index_remove_slot(slot);
    g_entries[e].target = NULL;
    if (g_entries[e].weak_count == 0) rx_entry_recycle(e);
}

rx_weak rx_weak_none(void) {
    rx_weak w = { RX_WEAK_NONE_INDEX, 0 };
    return w;
}

rx_weak rx_weak_ref(void* obj) {
    if (!obj) return rx_weak_none();
    rx_object* o = (rx_object*)obj;
    if (RX_UNLIKELY(o->refcount == 0)) {
        rx_plat_abort("rx_weak_ref: object is already deallocated");
    }
    uint32_t slot = rx_index_find_slot(obj);
    uint32_t e;
    if (slot != RX_WEAK_NONE_INDEX) {
        e = g_index[slot] - 1;
    } else {
        e = rx_entry_alloc();
        if (e == RX_WEAK_NONE_INDEX) {
            rx_plat_abort("rx_weak_ref: out of memory growing the weak table");
        }
        if (!rx_index_insert(obj, e)) {
            /* Give the entry back before aborting so the table stays consistent. */
            g_entries[e].next_free = g_free_head;
            g_free_head = e;
            rx_plat_abort("rx_weak_ref: out of memory growing the weak index");
        }
        g_entries[e].target = obj;
        g_entries[e].in_use = 1;
        g_live++;
    }
    if (RX_UNLIKELY(g_entries[e].weak_count >= RX_REFCOUNT_MAX)) {
        rx_plat_abort("rx_weak_ref: weak count overflow");
    }
    g_entries[e].weak_count++;
    rx_weak w = { e, g_entries[e].generation };
    return w;
}

rx_weak rx_weak_copy(rx_weak w) {
    if (!rx_weak_valid(w)) return rx_weak_none();
    if (RX_UNLIKELY(g_entries[w.index].weak_count >= RX_REFCOUNT_MAX)) {
        rx_plat_abort("rx_weak_copy: weak count overflow");
    }
    g_entries[w.index].weak_count++;
    return w;
}

void rx_weak_drop(rx_weak w) {
    if (!rx_weak_valid(w)) return;
    rx_weak_entry* e = &g_entries[w.index];
    if (e->weak_count > 0) e->weak_count--;
    if (e->weak_count == 0) {
        /* Nobody can load through this entry any more: recycle it now
         * (unregistering a still-live target) so the table never grows
         * with entries that have no handles. */
        if (e->target != NULL) {
            uint32_t slot = rx_index_find_slot(e->target);
            if (slot != RX_WEAK_NONE_INDEX) rx_index_remove_slot(slot);
        }
        rx_entry_recycle(w.index);
    }
}

void* rx_weak_load(rx_weak w) {
    if (!rx_weak_valid(w)) return NULL;
    void* target = g_entries[w.index].target;
    return target ? rx_retain(target) : NULL;
}

void rx_weak_assign(rx_weak* slot, void* value) {
    if (!slot) return;
    rx_weak fresh = rx_weak_ref(value);
    rx_weak old = *slot;
    *slot = fresh;
    rx_weak_drop(old);
}

bool rx_weak_alive(rx_weak w) {
    return rx_weak_valid(w) && g_entries[w.index].target != NULL;
}

size_t rx_weak_table_count(void) {
    return g_live;
}

void rx_weak_table_shutdown(void) {
    if (g_live != 0) return;
    rx_plat_free(g_entries);
    rx_plat_free(g_index);
    g_entries = NULL;
    g_index = NULL;
    g_entry_count = 0;
    g_entry_capacity = 0;
    g_index_capacity = 0;
    g_index_used = 0;
    g_free_head = RX_WEAK_NONE_INDEX;
}
