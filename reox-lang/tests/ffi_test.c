/**
 * REOX Hybrid FFI Test
 * Demonstrates C/C++ integration with REOX
 */

#include <stdio.h>
#include "reox_ffi.h"

/* Example C function that can be called from REOX */
int64_t my_add(int64_t a, int64_t b) {
    printf("C: my_add(%lld, %lld) = %lld\n", (long long)a, (long long)b, (long long)(a + b));
    return a + b;
}

int64_t my_print(int64_t msg_ptr) {
    const char* msg = (const char*)(uintptr_t)msg_ptr;
    printf("C: %s\n", msg);
    return 0;
}

/* Register C functions */
static RxFFIFunc c_functions[] = {
    {"my_add", (void*)my_add, "ii->i"},
    {"my_print", (void*)my_print, "s->v"},
};

int main(int argc, char* argv[]) {
    (void)argc; (void)argv;
    
    printf("=== REOX Hybrid FFI Test ===\n\n");
    
    /* Register C functions */
    rx_ffi_register(c_functions, 2);
    printf("Registered %d C functions for REOX\n", 2);
    
    /* Test calling FFI functions */
    int64_t args[] = {10, 32};
    int64_t result = rx_ffi_call("my_add", args, 2);
    printf("Result: %lld\n\n", (long long)result);
    
    /* Print message */
    int64_t msg_args[] = {(int64_t)(uintptr_t)"Hello from REOX!"};
    rx_ffi_call("my_print", msg_args, 1);
    
    /* Test app with null backend */
    printf("\nCreating app with null backend...\n");
    RxApp* app = rx_app_create("Test App", 800, 600, &rx_backend_null);
    printf("App created: %s\n", app ? "OK" : "FAILED");
    
    /* Clean up */
    rx_app_destroy(app);
    printf("App destroyed.\n");
    
    printf("\n=== FFI Test Complete ===\n");
    return 0;
}
