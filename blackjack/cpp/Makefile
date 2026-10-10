# Clang generates noticeably faster code for the AVX-512 engine (see ../README.md); GCC also works.
CXX := $(shell command -v clang++ 2>/dev/null || echo g++)
CXXFLAGS ?= -O3 -march=native -flto -std=c++17 -Wall -Wextra -pthread
TESTCXX ?= g++  # sanitizer runtimes ship with GCC; clang installs often lack them

all: bj

bj: main.cpp bj.hpp engine.hpp
	$(CXX) $(CXXFLAGS) -o $@ main.cpp

# Unit, equivalence and statistical tests, under AddressSanitizer + UBSan.
test: test.cpp bj.hpp engine.hpp
	$(TESTCXX) -O2 -g -march=native -std=c++17 -Wall -Wextra -fsanitize=address,undefined -fno-sanitize-recover=undefined -o bj_test test.cpp
	./bj_test

# Performance goal: 100M rounds in under 0.5 s on all cores (median of 5 runs).
perf: bj
	../perf_test.sh ./bj 0.5

clean:
	rm -f bj bj_test

.PHONY: all test perf clean
