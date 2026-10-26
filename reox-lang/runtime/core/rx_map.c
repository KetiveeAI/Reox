/*
 * REOX Runtime - string keyed map
 *
 * Entries are kept in insertion order in two parallel arrays (keys, values)
 * so iteration is deterministic. Lookup is a linear scan; see reox_runtime.h.
 */

#include "rx_core.h"

static void map_dealloc(void* p) {
    rx_map* m = (rx_map*)p;
    map_clear(m);
    rx_free(m->keys);
    rx_free(m->values);
    rx_free(m);
}

rx_map* map_new(size_t elem_size, const rx_elem_ops* ops) {
    if (elem_size == 0) return NULL;
    rx_map* m = (rx_map*)rx_alloc(sizeof(rx_map));
    if (!m) return NULL;
    rx_object_init(&m->header, RX_TYPE_MAP, map_dealloc);
    m->keys = NULL;
    m->values = NULL;
    m->len = 0;
    m->capacity = 0;
    m->elem_size = elem_size;
    m->ops = ops;
    return m;
}

size_t map_len(const rx_map* m) {
    return m ? m->len : 0;
}

static size_t map_find(const rx_map* m, const rx_str* key) {
    if (!m || !key) return SIZE_MAX;
    for (size_t i = 0; i < m->len; i++) {
        if (str_eq(m->keys[i], key)) return i;
    }
    return SIZE_MAX;
}

static bool map_reserve_one(rx_map* m) {
    if (m->len < m->capacity) return true;
    size_t new_capacity = m->capacity ? m->capacity * 2 : 8;
    if (new_capacity < m->capacity) return false;
    if (new_capacity > SIZE_MAX / sizeof(rx_str*) || new_capacity > SIZE_MAX / m->elem_size) return false;
    rx_str** keys = (rx_str**)rx_realloc(m->keys, new_capacity * sizeof(rx_str*));
    if (!keys) return false;
    m->keys = keys;
    void* values = rx_realloc(m->values, new_capacity * m->elem_size);
    if (!values) return false;          /* keys stay valid at the larger size */
    m->values = values;
    m->capacity = new_capacity;
    return true;
}

bool map_set(rx_map* m, rx_str* key, const void* value) {
    if (RX_UNLIKELY(!m || !key || !value)) return false;
    size_t i = map_find(m, key);
    if (i == SIZE_MAX) {
        if (!map_reserve_one(m)) return false;
        i = m->len++;
        m->keys[i] = (rx_str*)rx_retain(key);
        char* dest = (char*)m->values + i * m->elem_size;
        rx_memcpy(dest, value, m->elem_size);
        if (m->ops && m->ops->retain) m->ops->retain(dest);
        return true;
    }
    char* dest = (char*)m->values + i * m->elem_size;
    if (m->ops && m->ops->retain) {
        char tmp[64];
        if (m->elem_size <= sizeof tmp) {
            rx_memcpy(tmp, value, m->elem_size);
            m->ops->retain(tmp);
            m->ops->release(dest);
            rx_memcpy(dest, tmp, m->elem_size);
            return true;
        }
        m->ops->retain((void*)value);
        m->ops->release(dest);
    }
    rx_memcpy(dest, value, m->elem_size);
    return true;
}

void* map_get(rx_map* m, const rx_str* key) {
    size_t i = map_find(m, key);
    if (i == SIZE_MAX) return NULL;
    return (char*)m->values + i * m->elem_size;
}

bool map_has(const rx_map* m, const rx_str* key) {
    return map_find(m, key) != SIZE_MAX;
}

bool map_remove(rx_map* m, const rx_str* key) {
    size_t i = map_find(m, key);
    if (i == SIZE_MAX) return false;
    rx_release(m->keys[i]);
    char* slot = (char*)m->values + i * m->elem_size;
    if (m->ops && m->ops->release) m->ops->release(slot);
    size_t tail = m->len - i - 1;
    if (tail) {
        rx_memmove(&m->keys[i], &m->keys[i + 1], tail * sizeof(rx_str*));
        rx_memmove(slot, slot + m->elem_size, tail * m->elem_size);
    }
    m->len--;
    return true;
}

void map_clear(rx_map* m) {
    if (RX_UNLIKELY(!m)) return;
    for (size_t i = 0; i < m->len; i++) {
        rx_release(m->keys[i]);
        if (m->ops && m->ops->release) {
            m->ops->release((char*)m->values + i * m->elem_size);
        }
    }
    m->len = 0;
}

rx_array* map_keys(const rx_map* m) {
    rx_array* keys = array_new(sizeof(rx_str*), m && m->len ? m->len : 1, &rx_ref_elem_ops);
    if (!keys) return NULL;
    for (size_t i = 0; m && i < m->len; i++) {
        if (!array_push(keys, &m->keys[i])) {
            rx_release(keys);
            return NULL;
        }
    }
    return keys;
}
