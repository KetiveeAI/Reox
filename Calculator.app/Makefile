REOXC = ../reox-lang/target/debug/reoxc
RUNTIME = ../reox-lang/runtime
SRC = calculator.reox
OUT = calculator

.PHONY: all run clean

all: $(OUT)

# The runtime archive is no longer committed; build it first.
$(RUNTIME)/libreox_runtime.a:
	$(MAKE) -C $(RUNTIME) PLATFORM=linux

$(OUT): $(SRC) runtime_extensions.c $(RUNTIME)/libreox_runtime.a
	$(REOXC) $(SRC) --emit c -o $(OUT).c
	gcc -include shim.h $(OUT).c runtime_extensions.c reox_wrappers.c -o $(OUT) -I$(RUNTIME) -L$(RUNTIME) -Wl,--allow-multiple-definition -lreox_runtime -lm -lSDL2

run: $(OUT)
	LD_LIBRARY_PATH=$(LD_LIBRARY_PATH):../reox-lang/runtime ./$(OUT)

clean:
	rm -f $(OUT) $(OUT).c
