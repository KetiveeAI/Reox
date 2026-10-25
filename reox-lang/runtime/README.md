# REOX runtime (librx)

The C runtime that REOX programs link against. It is split along a single
platform seam so the same code builds for the Linux development host and for
NeolyxOS's freestanding userspace.

```
runtime/
  core/                 OS independent: strings, arrays, formatting, UI model,
                        animation, transitions, themes, colour system, state
                        diffing, FFI registry, project scaffolding templates.
                        May include only <stdint.h> <stddef.h> <stdbool.h>
                        <stdarg.h>. All memory, I/O, time, math and display
                        access goes through platform/rx_platform.h.
  platform/rx_platform.h   The contract: rx_plat_alloc/realloc/free,
                        rx_plat_write/read/open/close/seek/stat, filesystem
                        and process extras, rx_plat_time_ms/sleep_ms/exit/abort,
                        a double-precision math shim, and the framebuffer and
                        input shim (rx_plat_fb_*, rx_plat_input_poll).
  platform/linux/       glibc/libm implementation; SDL2 is optional and gives
                        the framebuffer shim a window. reox_backend_sdl.c and
                        reox_sdl_visual.c (SDL_ttf harness) live here too.
  platform/neolyx/      nxsyscall.h + nx_malloc implementation, a freestanding
                        libm subset (rx_math_neolyx.c, accuracy documented at
                        its top), weak memcpy/memset/memmove/memcmp for
                        compiler-generated calls, and the nxgfx-backed slider
                        and colour picker widgets.
  tests/                Host-side checks: the libc subset against glibc, the
                        math shim against libm (make test).
  reox_runtime.h, reox_nxrender_bridge.h
                        Forwarding headers so -I<runtime> consumers keep working.
```

## Building

```
make                    # librx_linux.a (+ libreox_runtime.a compatibility copy)
make PLATFORM=neolyx    # librx_neolyx.a with the NeolyxOS userspace flags
make check              # fail if core/ includes a libc or OS header
make test               # host tests (Linux)
```

`NEOLYX_ROOT` (default `/home/swa/NEOLYXOS/neolyx-os`) points at the NeolyxOS
checkout for `desktop/include` and `gui/nxrender_c/include`.

Both targets build warning-free with `-Wall -Wextra`. The NeolyxOS target adds
`-fno-tree-loop-distribute-patterns` so GCC never rewrites the runtime's own
byte loops back into `memcpy`/`memset` calls.

## Rules for code under core/

- No libc. Use `rx_strlen`, `rx_memcpy`, `rx_snprintf`, `rx_printf`,
  `rx_strdup`, `rx_sort`, `rx_rand` and friends from `reox_runtime.h`.
- Every allocation is checked. Constructors return NULL and container
  mutators return `false`/`-1` on failure; nothing dereferences a NULL result.
- Every `rx_plat_*` call checks its return value.

## What the NeolyxOS platform cannot do yet

`rx_plat_mkdir/unlink/rmdir/rename/getenv/setenv/exec/chdir` return
`RX_PLAT_ENOSYS` because the kernel has no such syscalls. `nx_mfree` is a
no-op in the current `nx_malloc`, so freed memory is not reclaimed.
