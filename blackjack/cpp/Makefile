CXX ?= g++
CXXFLAGS ?= -O3 -march=native -flto -std=c++17 -Wall -Wextra -pthread

all: bj

bj: main.cpp bj.hpp
	$(CXX) $(CXXFLAGS) -o $@ main.cpp

test: test.cpp bj.hpp
	$(CXX) -O2 -std=c++17 -Wall -Wextra -fsanitize=address,undefined -o bj_test test.cpp
	./bj_test

clean:
	rm -f bj bj_test

.PHONY: all test clean
