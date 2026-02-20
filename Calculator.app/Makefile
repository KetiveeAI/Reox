REOXC = ../reox-lang/target/debug/reoxc
SRC = calculator.reox
OUT = calculator

.PHONY: all run clean

all: $(OUT)

$(OUT): $(SRC) runtime_extensions.c
	$(REOXC) $(SRC) --emit c -o $(OUT).c
	gcc -include shim.h $(OUT).c runtime_extensions.c reox_wrappers.c -o $(OUT) -I../reox-lang/runtime -L../reox-lang/runtime -Wl,--allow-multiple-definition -lreox_runtime -lm -lSDL2

run: $(OUT)
	LD_LIBRARY_PATH=$(LD_LIBRARY_PATH):../reox-lang/runtime ./$(OUT)

clean:
	rm -f $(OUT) $(OUT).c
