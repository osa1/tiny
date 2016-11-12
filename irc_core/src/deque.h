#ifndef __DEQUE_H
#define __DEQUE_H

typedef struct deque_ deque;

deque* deque_new(int initial_cap);
void   deque_free(deque*);

int deque_size(deque*);

void deque_push_front(deque*, void*);
void deque_push_back(deque*, void*);

void* deque_pop_front(deque*);
void* deque_pop_back(deque*);

#endif
