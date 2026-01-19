# REOX Runtime Library Makefile
# Optimized for LTO and inlining
# Enhanced with transitions, color system, and stdlib wrappers

CC = gcc
CFLAGS = -Wall -Wextra -O3 -fPIC -flto -ffunction-sections -fdata-sections -Wno-unused-parameter -Wno-unused-variable
LDFLAGS = -lm -Wl,--gc-sections

# Check for NXRender
NXRENDER_PATH = ../../../gui/nxrender_c/include
NXRENDER_EXISTS = $(shell test -d $(NXRENDER_PATH) && echo "yes")

# Core source files
CORE_SRC = reox_runtime.c reox_ui.c reox_wrappers.c reox_animation.c reox_theme.c
CORE_OBJ = reox_runtime.o reox_ui.o reox_wrappers.o reox_animation.o reox_theme.o

# Extended modules source files
EXT_SRC = reox_transitions.c reox_color_system.c
EXT_OBJ = reox_transitions.o reox_color_system.o

# Optional modules (require NXRender)
ifeq ($(NXRENDER_EXISTS),yes)
  OPT_SRC = reox_slider.c reox_colorpicker.c
  OPT_OBJ = reox_slider.o reox_colorpicker.o
  OPT_FLAGS = -I$(NXRENDER_PATH)
else
  OPT_SRC =
  OPT_OBJ =
  OPT_FLAGS =
endif

# FFI module
FFI_SRC = reox_ffi.c
FFI_OBJ = reox_ffi.o

# All objects
RUNTIME_OBJ = $(CORE_OBJ) $(EXT_OBJ) $(OPT_OBJ) $(FFI_OBJ)
RUNTIME_LIB = libreox_runtime.a

# Stdlib wrapper headers (header-only, no compilation needed)
STDLIB_HEADERS = reox_transition_stdlib.h reox_nxrender_stdlib.h reox_system_stdlib.h

# Default target
all: $(RUNTIME_LIB)
	@echo "Built $(RUNTIME_LIB) with $(words $(RUNTIME_OBJ)) modules"
	@echo "Stdlib wrappers: $(STDLIB_HEADERS)"

# Core module objects
reox_runtime.o: reox_runtime.c reox_runtime.h
	$(CC) $(CFLAGS) -c reox_runtime.c -o reox_runtime.o

reox_ui.o: reox_ui.c reox_ui.h reox_runtime.h
	$(CC) $(CFLAGS) -c reox_ui.c -o reox_ui.o

reox_wrappers.o: reox_wrappers.c reox_ui.h reox_runtime.h
	$(CC) $(CFLAGS) -c reox_wrappers.c -o reox_wrappers.o

reox_animation.o: reox_animation.c reox_animation.h reox_ui.h
	$(CC) $(CFLAGS) -c reox_animation.c -o reox_animation.o

reox_theme.o: reox_theme.c reox_theme.h reox_ui.h
	$(CC) $(CFLAGS) -c reox_theme.c -o reox_theme.o

# Extended module objects
reox_transitions.o: reox_transitions.c reox_transitions.h reox_animation.h reox_ui.h
	$(CC) $(CFLAGS) -c reox_transitions.c -o reox_transitions.o

reox_color_system.o: reox_color_system.c reox_color_system.h reox_ui.h
	$(CC) $(CFLAGS) -c reox_color_system.c -o reox_color_system.o

# Optional modules (require NXRender)
ifeq ($(NXRENDER_EXISTS),yes)
reox_slider.o: reox_slider.c reox_slider.h reox_ui.h
	$(CC) $(CFLAGS) $(OPT_FLAGS) -c reox_slider.c -o reox_slider.o

reox_colorpicker.o: reox_colorpicker.c reox_colorpicker.h reox_slider.h reox_ui.h
	$(CC) $(CFLAGS) $(OPT_FLAGS) -c reox_colorpicker.c -o reox_colorpicker.o
endif

# FFI module
reox_ffi.o: reox_ffi.c reox_ffi.h
	$(CC) $(CFLAGS) -c reox_ffi.c -o reox_ffi.o

# Create static library (LTO-compatible)
$(RUNTIME_LIB): $(RUNTIME_OBJ)
	ar rcs $(RUNTIME_LIB) $(RUNTIME_OBJ)

# Install headers
install-headers:
	mkdir -p /usr/local/include/reox
	cp *.h /usr/local/include/reox/

# Clean build artifacts
clean:
	rm -f $(CORE_OBJ) $(EXT_OBJ) $(OPT_OBJ) $(FFI_OBJ) $(RUNTIME_LIB) libreox_runtime.so

.PHONY: all clean install-headers

