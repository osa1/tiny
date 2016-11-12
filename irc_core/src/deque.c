#include <assert.h>
#include <stdlib.h>

#include "deque.h"

typedef struct deque_
{
    void** buf;
    int    cap;
    int    front; //< Empty slot
    int    size;
} deque;

static int find_back(deque*);

/*
void deque_print(deque* d)
{
    int back = find_back(d) + 1;

    printf("[ ");
    for (int i = 0; i < d->size; ++i)
    {
        int idx = (back + i) % d->cap;
        printf("%p", d->buf[idx]);
        if (i != d->size - 1)
            printf(",");
    }
    printf(" ]\n");
}
*/

deque* deque_new(int initial_cap)
{
    deque* d = malloc(sizeof(deque));

    d->buf   = malloc(sizeof(void*) * initial_cap);
    d->cap   = initial_cap;
    d->front = 0;
    d->size  = 0;

    return d;
}

void deque_free(deque* d)
{
    free(d->buf);
    free(d);
}

int deque_size(deque* d)
{
    return d->size;
}

void deque_reserve(deque* d)
{
    assert(d->size <= d->cap);
    assert(d->cap != 0);
    if (d->size == d->cap)
    {
        int new_cap = d->cap * 2;
        void** new_buf = malloc(sizeof(void*) * new_cap);

        int i = 0;
        int back = (find_back(d) + 1) % d->cap;
        while (i < d->size)
        {
            new_buf[i] = d->buf[(back + i) % d->cap];
            ++i;
        }

        free(d->buf);

        d->buf   = new_buf;
        d->cap   = new_cap;
        d->front = i;
    }
}

// Find back of the queue.
static int find_back(deque* d)
{
    int back = (d->front - d->size - 1) % d->cap;
    if (back < 0) back += d->cap;
    assert(back >= 0 && back < d->cap);
    return back;
}

void deque_push_front(deque* d, void* p)
{
    deque_reserve(d);
    d->buf[d->front] = p;
    d->front = (d->front + 1) % d->cap;
    ++d->size;
}

void deque_push_back(deque* d, void* p)
{
    deque_reserve(d);
    d->buf[find_back(d)] = p;
    ++d->size;
}

void* deque_pop_front(deque* d)
{
    assert(d->front >= 0 && d->front < d->cap);

    if (d->size == 0) return NULL;

    --d->front;
    if (d->front < 0)
        d->front += d->cap;
    void* ret = d->buf[d->front];

    --d->size;

    return ret;
}

void *deque_pop_back(deque* d)
{
    if (d->size == 0) return NULL;

    int back = find_back(d);
    void* ret = d->buf[(back + 1) % d->cap];
    --d->size;

    return ret;
}
