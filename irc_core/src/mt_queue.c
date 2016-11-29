#include "mt_queue.h"
#include "mt_deque.h"

#include <stdlib.h>

typedef struct mt_queue_
{
    mt_deque* deque;
} mt_queue;

mt_queue* mt_queue_new(int initial_cap)
{
    mt_queue* ret = malloc(sizeof(mt_queue));
    ret->deque = mt_deque_new(initial_cap);
    return ret;
}

void mt_queue_free(mt_queue* q)
{
    mt_deque_free(q->deque);
    free(q);
}

void mt_queue_push(mt_queue* q, void* t)
{
    mt_deque_push_back(q->deque, t);
}

void* mt_queue_pop(mt_queue* q)
{
    return mt_deque_pop_front(q->deque);
}

int mt_queue_get_eventfd(mt_queue* q)
{
    return mt_deque_get_push_eventfd(q->deque);
}
