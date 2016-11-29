#ifndef __MT_DEQUE_H
#define __MT_DEQUE_H

/**
 * Multiple-producer, single-consumer thread-safe deque.
 *
 * 'mt_deque_push_*' are safe to use multi-threaded.
 * 'mt_deque_pop_*' are not safe for multi-threaded use.
 */

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
