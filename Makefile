CC 			= clang
CC_FLAGS 	= -std=gnu11

objs = main.o textfield.o textarea.o

tiny: $(objs)
	$(CC) $^ -o $@ -g -lncurses

%.o: src/%.c
	$(CC) -Iinclude $^ -c -o $@ -g -Wall -Wpedantic -Wextra $(CC_FLAGS)

.PHONE: clean

clean:
	rm -f tiny
	rm -f *.o
