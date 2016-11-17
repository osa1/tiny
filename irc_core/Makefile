CC=gcc -std=gnu99

ALL: deque.o mt_deque.o test irc_core.o msg_buf.o test_conn mainloop message.o test_parsing

deque.o: src/deque.c src/deque.h
	$(CC) $< -c -o $@ -Wall -Wextra -g -lpthread

test: test.c deque.o mt_deque.o msg_buf.o
	$(CC) $^ -o $@ -Wall -Wextra -g -lpthread

test_conn: test_conn.c deque.o mt_deque.o msg_buf.o
	$(CC) $^ -o $@ -Wall -Wextra -g -lpthread

mainloop: mainloop.c deque.o mt_deque.o msg_buf.o message.o
	$(CC) $^ -o $@ -Wall -Wextra -g -lpthread

mt_deque.o: src/mt_deque.c src/mt_deque.h deque.o
	$(CC) $< -c -o $@ -Wall -Wextra -g -lpthread

irc_core.o: src/irc_core.c
	$(CC) $< -c -o $@ -Wall -Wextra -g -lpthread

msg_buf.o: src/msg_buf.c src/msg_buf.h
	$(CC) $< -c -o $@ -Wall -Wextra -g

message.o: src/message.c src/message.h
	$(CC) $< -c -o $@ -Wall -Wextra -g

test_parsing: test_parsing.c message.o
	$(CC) $^ -o $@ -Wall -Wextra -g -lpthread

.PHONY: clean
clean:
	rm -f *.o test test_conn mainloop test_parsing
