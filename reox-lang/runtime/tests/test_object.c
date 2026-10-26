/*
 * Host-side checks for the reference counting header and the weak table in
 * core/rx_object.c. The overflow and double-release aborts are exercised in
 * forked children so that this process can observe the abnormal exit.
 * Linked against librx_linux.a. Exit code 0 means every check passed.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>
#include <fcntl.h>
#include <signal.h>
#include "../core/reox_runtime.h"

static int failures = 0;

#define CHECK(cond) do { if (!(cond)) { printf("FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); failures++; } } while (0)

typedef struct node {
    rx_object header;
    int* dealloc_counter;
    struct node* next;      /* strong */
    rx_weak back;           /* weak */
} node;

static void node_dealloc(void* p) {
    node* n = (node*)p;
    (*n->dealloc_counter)++;
    rx_release(n->next);
    rx_weak_drop(n->back);
    rx_free(n);
}

static node* node_new(int* counter) {
    node* n = (node*)rx_calloc(1, sizeof(node));
    if (!n) abort();
    rx_object_init(&n->header, rx_type_id_hash("node"), node_dealloc);
    n->dealloc_counter = counter;
    n->back = rx_weak_none();
    return n;
}

/* Run fn in a child; return true when the child died from SIGABRT. */
static bool dies_with_abort(void (*fn)(void)) {
    fflush(NULL);
    pid_t pid = fork();
    if (pid < 0) return false;
    if (pid == 0) {
        int devnull = open("/dev/null", 0);
        if (devnull >= 0) dup2(devnull, 2);
        fn();
        _exit(0);
    }
    int status = 0;
    if (waitpid(pid, &status, 0) < 0) return false;
    return WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT;
}

static void overflow_refcount(void) {
    int counter = 0;
    node* n = node_new(&counter);
    n->header.refcount = RX_REFCOUNT_MAX;
    rx_retain(n);                       /* must abort, never wrap */
}

static void double_release(void) {
    int counter = 0;
    node* n = node_new(&counter);
    n->header.refcount = 2;
    rx_release(n);
    n->header.refcount = 0;             /* simulate the object being gone */
    rx_release(n);                      /* must abort */
}

static void weak_count_overflow(void) {
    int counter = 0;
    node* n = node_new(&counter);
    rx_weak w = rx_weak_ref(n);
    for (uint32_t i = 0; i < RX_REFCOUNT_MAX; i++) rx_weak_copy(w);
    rx_weak_copy(w);                    /* one past the limit must abort */
}

int main(void) {
    /* header basics */
    int counter = 0;
    node* a = node_new(&counter);
    CHECK(rx_refcount(a) == 1);
    CHECK(rx_type_id(a) == rx_type_id_hash("node"));
    CHECK((rx_type_id_hash("node") & 0x80000000u) != 0);
    CHECK(rx_type_id_hash("node") != rx_type_id_hash("Node"));
    CHECK(rx_retain(a) == a && rx_refcount(a) == 2);
    rx_release(a);
    CHECK(rx_refcount(a) == 1 && counter == 0);
    CHECK(rx_retain(NULL) == NULL);
    rx_release(NULL);
    CHECK(rx_refcount(NULL) == 0 && rx_type_id(NULL) == RX_TYPE_NONE);

    /* rx_assign / rx_clear */
    node* slot = NULL;
    rx_assign((void**)&slot, a);
    CHECK(slot == a && rx_refcount(a) == 2);
    rx_assign((void**)&slot, a);            /* self assignment */
    CHECK(rx_refcount(a) == 2);
    rx_clear((void**)&slot);
    CHECK(slot == NULL && rx_refcount(a) == 1);

    /* strong chain: releasing the head frees the whole chain in order */
    node* b = node_new(&counter);
    a->next = (node*)rx_retain(b);
    rx_release(b);
    CHECK(rx_refcount(b) == 1 && counter == 0);
    rx_release(a);
    CHECK(counter == 2);

    /* weak references zero out when the target dies */
    counter = 0;
    node* c = node_new(&counter);
    rx_weak w1 = rx_weak_ref(c);
    rx_weak w2 = rx_weak_ref(c);
    CHECK(w1.index == w2.index && w1.generation == w2.generation);
    CHECK(rx_weak_table_count() == 1);
    CHECK(rx_weak_alive(w1));
    node* loaded = (node*)rx_weak_load(w1);
    CHECK(loaded == c && rx_refcount(c) == 2);
    rx_release(loaded);
    rx_release(c);
    CHECK(counter == 1);
    CHECK(!rx_weak_alive(w1));
    CHECK(rx_weak_load(w1) == NULL && rx_weak_load(w2) == NULL);
    CHECK(rx_weak_table_count() == 1);      /* entry lives until the handles drop */
    rx_weak_drop(w1);
    CHECK(rx_weak_table_count() == 1);
    rx_weak_drop(w2);
    CHECK(rx_weak_table_count() == 0);
    rx_weak_drop(w2);                       /* stale handle: no effect */
    CHECK(rx_weak_table_count() == 0);

    /* a recycled entry gets a new generation so stale handles stay dead */
    node* d = node_new(&counter);
    rx_weak w3 = rx_weak_ref(d);
    CHECK(w3.index == w1.index && w3.generation != w1.generation);
    CHECK(rx_weak_load(w1) == NULL);
    CHECK(rx_weak_alive(w3));
    rx_weak_drop(w3);
    CHECK(rx_weak_table_count() == 0);      /* dropping the last handle while alive recycles too */
    rx_weak w4 = rx_weak_ref(d);
    rx_release(d);
    CHECK(!rx_weak_alive(w4));
    rx_weak_drop(w4);

    /* parent <-> child cycle broken by a weak back pointer */
    counter = 0;
    node* parent = node_new(&counter);
    node* child = node_new(&counter);
    parent->next = (node*)rx_retain(child);
    child->back = rx_weak_ref(parent);
    rx_release(child);
    node* back = (node*)rx_weak_load(child->back);
    CHECK(back == parent);
    rx_release(back);
    rx_release(parent);
    CHECK(counter == 2);
    CHECK(rx_weak_table_count() == 0);

    /* a strong cycle is NOT collected: documented programmer responsibility */
    counter = 0;
    node* x = node_new(&counter);
    node* y = node_new(&counter);
    x->next = (node*)rx_retain(y);
    y->next = (node*)rx_retain(x);
    rx_release(x);
    rx_release(y);
    CHECK(counter == 0);
    /* break it by hand so this test leaks nothing */
    node* keep = y->next;                   /* x */
    y->next = NULL;
    rx_release(keep);
    CHECK(counter == 2);

    /* rx_weak_assign */
    counter = 0;
    node* e = node_new(&counter);
    node* f = node_new(&counter);
    rx_weak wslot = rx_weak_none();
    rx_weak_assign(&wslot, e);
    rx_weak_assign(&wslot, f);
    CHECK(rx_weak_table_count() == 1);
    node* got = (node*)rx_weak_load(wslot);
    CHECK(got == f);
    rx_release(got);
    rx_weak_assign(&wslot, NULL);
    CHECK(rx_weak_table_count() == 0);
    rx_release(e); rx_release(f);

    /* many weak references force the table and index to grow and rehash */
    counter = 0;
    enum { N = 1000 };
    node* many[N];
    rx_weak handles[N];
    for (int i = 0; i < N; i++) { many[i] = node_new(&counter); handles[i] = rx_weak_ref(many[i]); }
    CHECK(rx_weak_table_count() == N);
    for (int i = 0; i < N; i += 2) rx_release(many[i]);
    for (int i = 0; i < N; i++) {
        node* l = (node*)rx_weak_load(handles[i]);
        CHECK((i % 2 == 0) ? (l == NULL) : (l == many[i]));
        rx_release(l);
    }
    for (int i = 1; i < N; i += 2) rx_release(many[i]);
    for (int i = 0; i < N; i++) rx_weak_drop(handles[i]);
    CHECK(counter == N && rx_weak_table_count() == 0);

    /* immortal objects are never counted */
    static rx_str lit = RX_STR_LIT_INIT("immortal");
    rx_retain(&lit); rx_release(&lit); rx_release(&lit);
    CHECK(rx_refcount(&lit) == RX_REFCOUNT_IMMORTAL);

    /* hard aborts */
    CHECK(dies_with_abort(overflow_refcount));
    CHECK(dies_with_abort(double_release));
    CHECK(dies_with_abort(weak_count_overflow));

    rx_weak_table_shutdown();
    printf("%s (%d failures)\n", failures ? "test_object: FAILED" : "test_object: ok", failures);
    return failures ? 1 : 0;
}
