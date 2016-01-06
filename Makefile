CC=clang

tiny: src/main.c
	$(CC) $^ -o $@ -g -lncurses -Wall -Wpedantic

.PHONE: clean

clean:
	rm tiny
