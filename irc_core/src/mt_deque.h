#ifndef __MT_DEQUE_H
#define __MT_DEQUE_H

// Single-producer, single-consumer thread-safe deque.

// This can be made multi-producer, multi-consumer by wrapping push and pop
// ends of the queue with a mutex, but we don't need this in this program.

#include <stdbool.h>

typedef struct mt_deque_ mt_deque;

mt_deque* mt_deque_new(int initial_cap);
void mt_deque_free(mt_deque*);

void mt_deque_push_front(mt_deque*, void*);
void mt_deque_push_back(mt_deque*, void*);

void* mt_deque_pop_front(mt_deque*);
void* mt_deque_pop_back(mt_deque*);

bool mt_deque_try_pop_front(mt_deque* md, void** ret);
bool mt_deque_try_pop_back(mt_deque* md, void** ret);

int mt_deque_get_push_eventfd(mt_deque*);

#endif
