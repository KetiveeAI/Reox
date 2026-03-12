// REOX FFI Bridge Header
// Conventions for calling C/C++/Rust from REOX and vice versa
//
// REOX uses the C ABI by default. Any function declared with
// `extern fn` in .rx code emits a standard C extern declaration.
// This header provides macros for the native side.

#ifndef REOX_FFI_BRIDGE_H
#define REOX_FFI_BRIDGE_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

// ---------------------------------------------------------------------------
// Visibility macros
// ---------------------------------------------------------------------------

#ifdef _WIN32
  #define REOX_EXPORT __declspec(dllexport)
  #define REOX_IMPORT __declspec(dllimport)
#else
  #define REOX_EXPORT __attribute__((visibility("default")))
  #define REOX_IMPORT extern
#endif

// Use REOX_API on functions you want REOX to call
#ifdef REOX_BUILDING
  #define REOX_API REOX_EXPORT
#else
  #define REOX_API REOX_IMPORT
#endif

// ---------------------------------------------------------------------------
// Type aliases matching REOX type system → C
// ---------------------------------------------------------------------------

typedef int64_t   rx_int;
typedef double    rx_float;
typedef const char* rx_string;
typedef bool      rx_bool;
typedef int8_t    rx_i8;
typedef uint8_t   rx_u8;
typedef int16_t   rx_i16;
typedef uint16_t  rx_u16;
typedef int32_t   rx_i32;
typedef uint32_t  rx_u32;
typedef int64_t   rx_i64;
typedef uint64_t  rx_u64;
typedef size_t    rx_usize;

// ---------------------------------------------------------------------------
// C++ interop
// ---------------------------------------------------------------------------
// In C++ files, use:
//   extern "C" { #include "reox_ffi_bridge.h" }
// Or mark individual functions:
//   extern "C" REOX_API int32_t my_cpp_function(rx_int arg);

// ---------------------------------------------------------------------------
// Rust interop
// ---------------------------------------------------------------------------
// In Rust, export functions callable from REOX like this:
//
//   #[no_mangle]
//   pub extern "C" fn my_rust_function(arg: i64) -> i32 { ... }
//
// Then in REOX:
//   extern fn my_rust_function(arg: int) -> i32;

// ---------------------------------------------------------------------------
// Calling convention notes
// ---------------------------------------------------------------------------
// - REOX `int` is always 64-bit signed (int64_t / i64)
// - REOX `float` is always 64-bit (double / f64)
// - REOX `string` is a null-terminated const char*
// - REOX `bool` is C99 _Bool / bool
// - Structs are passed by value (matching C struct ABI)
// - Arrays are passed as pointer + length (rx_array_t)

#endif // REOX_FFI_BRIDGE_H
