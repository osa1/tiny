ALL: deque.o mt_deque.o test irc_core.o msg_buf.o

deque.o: src/deque.c src/deque.h
	gcc $< -c -o $@ -Wall -Wextra -g -lpthread

test: test.c deque.o mt_deque.o msg_buf.o
	gcc $^ -o $@ -Wall -Wextra -g -lpthread

mt_deque.o: src/mt_deque.c src/mt_deque.h deque.o
	gcc $< -c -o $@ -Wall -Wextra -g -lpthread

irc_core.o: src/irc_core.c
	gcc $< -c -o $@ -Wall -Wextra -g -lpthread

msg_buf.o: src/msg_buf.c src/msg_buf.h
	gcc $< -c -o $@ -Wall -Wextra -g

.PHONY: clean
clean:
	rm -f *.o test
