# REOX Runtime Library (librx)
#
#   make                      -> librx_linux.a  (+ libreox_runtime.a compat copy)
#   make PLATFORM=neolyx      -> librx_neolyx.a (freestanding, NeolyxOS flags)
#   make check                -> fail if core/ includes anything but the
#                                four freestanding headers
#   make clean
#
# Layout:
#   core/             OS independent runtime (strings, arrays, UI model, ...)
#   platform/         rx_platform.h contract + one implementation per OS
#   platform/linux/   libc/libm/SDL2 implementation and the SDL backends
#   platform/neolyx/  nxsyscall.h + nx_malloc implementation, nxrender widgets

PLATFORM ?= linux
CC       ?= gcc
AR       ?= ar
NEOLYX_ROOT ?= /home/swa/NEOLYXOS/neolyx-os

BUILD_DIR := build/$(PLATFORM)

# Core is identical for every platform.
CORE_SRC := $(sort $(wildcard core/*.c))

# Warnings are the same everywhere; both platforms must build clean.
WARNFLAGS := -Wall -Wextra

ifeq ($(PLATFORM),linux)
  LIB      := librx_linux.a
  CFLAGS   := $(WARNFLAGS) -std=c11 -O2 -fPIC -ffunction-sections -fdata-sections
  PLAT_SRC := platform/linux/rx_platform_linux.c platform/linux/reox_backend_sdl.c
  # SDL2 is optional on the host: used for the framebuffer shim and rx_backend_sdl.
  SDL_CFLAGS := $(shell pkg-config --cflags sdl2 2>/dev/null)
  SDL_FOUND  := $(shell pkg-config --exists sdl2 2>/dev/null && echo yes)
  ifeq ($(SDL_FOUND),yes)
    CFLAGS += -DRX_HAVE_SDL -DREOX_USE_SDL $(SDL_CFLAGS)
  endif
else ifeq ($(PLATFORM),neolyx)
  LIB      := librx_neolyx.a
  # Exactly the NeolyxOS userspace flags (desktop/Makefile), plus
  # -fno-tree-loop-distribute-patterns so GCC never turns the runtime's own
  # byte loops back into memcpy/memset calls.
  CFLAGS   := $(WARNFLAGS) -std=c11 -O2 \
              -ffreestanding -fno-stack-protector -mno-red-zone -nostdlib \
              -D__NEOLYXOS__ -fno-tree-loop-distribute-patterns \
              -I$(NEOLYX_ROOT)/desktop/include \
              -I$(NEOLYX_ROOT)/gui/nxrender_c/include
  PLAT_SRC := $(sort $(wildcard platform/neolyx/*.c))
else
  $(error PLATFORM must be linux or neolyx, got '$(PLATFORM)')
endif

SRC := $(CORE_SRC) $(PLAT_SRC)
OBJ := $(patsubst %.c,$(BUILD_DIR)/%.o,$(SRC))
DEP := $(OBJ:.o=.d)

.PHONY: all clean check sdl-visual

all: $(LIB)

$(LIB): $(OBJ)
	$(AR) rcs $@ $^
ifeq ($(PLATFORM),linux)
	cp $@ libreox_runtime.a
endif
	@echo "Built $@ ($(words $(OBJ)) objects, platform=$(PLATFORM))"

$(BUILD_DIR)/%.o: %.c
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -MMD -MP -c $< -o $@

# Standalone SDL2 + SDL_ttf visual harness (Linux only, not part of the
# archive because it redefines the print/UI wrapper symbols).
sdl-visual: $(BUILD_DIR)/platform/linux/reox_sdl_visual.o
$(BUILD_DIR)/platform/linux/reox_sdl_visual.o: platform/linux/reox_sdl_visual.c
	@mkdir -p $(dir $@)
	$(CC) $(WARNFLAGS) -std=c11 -O2 -Icore $(shell pkg-config --cflags sdl2 SDL2_ttf) -c $< -o $@

# Fail if anything under core/ reaches for a libc or OS header.
ALLOWED_INCLUDES := stdint.h stddef.h stdbool.h stdarg.h
check:
	@bad=$$(grep -nE '^[[:space:]]*#[[:space:]]*include[[:space:]]*<' core/*.c core/*.h \
	    | grep -vE '<($(subst $(eval) ,|,$(subst .,\.,$(ALLOWED_INCLUDES))))>' || true); \
	if [ -n "$$bad" ]; then \
	    echo "make check: forbidden includes in core/:"; echo "$$bad"; exit 1; \
	fi; \
	bad=$$(grep -nE '\b(malloc|calloc|realloc|free|printf|fprintf|sprintf|snprintf|puts|fputs|fopen|strlen|strcpy|strncpy|strcat|strcmp|memcpy|memset|strdup|qsort|exit|abort)[[:space:]]*\(' core/*.c core/*.h \
	    | grep -vE '\brx_[a-z_]*\(' | grep -vE '^[^:]*:[0-9]+:[[:space:]]*(/?\*|//)' || true); \
	if [ -n "$$bad" ]; then \
	    echo "make check: direct libc calls in core/:"; echo "$$bad"; exit 1; \
	fi; \
	echo "make check: core/ is freestanding"

clean:
	rm -rf build librx_linux.a librx_neolyx.a libreox_runtime.a

-include $(DEP)

# ---------------------------------------------------------------------------
# Host-side tests (Linux only): the freestanding libc subset against glibc,
# and the NeolyxOS math shim against libm.
.PHONY: test mathcheck
TEST_LIBS := librx_linux.a -lm $(shell pkg-config --libs sdl2 2>/dev/null)
test: librx_linux.a tests/test_core.c tests/test_object.c
	@mkdir -p $(BUILD_DIR)/tests
	$(CC) $(WARNFLAGS) -std=c11 -O2 -Wno-format-truncation tests/test_core.c $(TEST_LIBS) -o $(BUILD_DIR)/tests/test_core
	$(BUILD_DIR)/tests/test_core
	$(CC) $(WARNFLAGS) -std=gnu11 -O2 tests/test_object.c $(TEST_LIBS) -o $(BUILD_DIR)/tests/test_object
	$(BUILD_DIR)/tests/test_object
	$(MAKE) mathcheck

mathcheck: tests/test_math.c platform/neolyx/rx_math_neolyx.c
	@mkdir -p $(BUILD_DIR)/tests
	$(CC) $(WARNFLAGS) -std=c11 -O2 -fno-builtin tests/test_math.c platform/neolyx/rx_math_neolyx.c -lm -o $(BUILD_DIR)/tests/test_math
	$(BUILD_DIR)/tests/test_math
