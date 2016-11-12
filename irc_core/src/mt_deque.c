#include <assert.h>
#include <poll.h>
#include <pthread.h>
#include <stdint.h>
#include <stdlib.h>
#include <sys/eventfd.h>
#include <unistd.h>

#include "deque.h"
#include "mt_deque.h"

#define assert0(f) \
    { \
        int __ret = f; \
        assert(__ret == 0); \
    }

// Little-endian, 8-byte one.
static uint8_t ONE_LE[8] = { 1, 0, 0, 0, 0, 0, 0, 0 };

typedef struct mt_deque_
{
    deque*              d;
    pthread_mutex_t     d_mutex;
    int                 push_eventfd;
} mt_deque;

bool poll_push_eventfd(mt_deque* md)
{
    struct pollfd fds[1];
    fds[0].fd = md->push_eventfd;
    fds[0].events = POLLIN;
    return poll(fds, 1, 0) == 1;
}

int mt_deque_get_push_eventfd(mt_deque* md)
{
    return md->push_eventfd;
}

mt_deque* mt_deque_new(int initial_cap)
{
    mt_deque* md = malloc(sizeof(mt_deque));
    md->d = deque_new(initial_cap);
    pthread_mutex_init(&md->d_mutex, 0);
    md->push_eventfd = eventfd(0, EFD_SEMAPHORE);
    return md;
}

void mt_deque_free(mt_deque* md)
{
    deque_free(md->d);
    pthread_mutex_destroy(&md->d_mutex);
    close(md->push_eventfd);
    free(md);
}

void mt_deque_push_front(mt_deque* md, void* p)
{
    assert0(pthread_mutex_lock(&md->d_mutex));
    deque_push_front(md->d, p);
    assert0(pthread_mutex_unlock(&md->d_mutex));
    write(md->push_eventfd, ONE_LE, 8);
}

void mt_deque_push_back(mt_deque* md, void* p)
{
    assert0(pthread_mutex_lock(&md->d_mutex));
    deque_push_back(md->d, p);
    assert0(pthread_mutex_unlock(&md->d_mutex));
    write(md->push_eventfd, ONE_LE, 8);
}

void* mt_deque_pop_front(mt_deque* md)
{
    uint8_t read_buf[8];
    read(md->push_eventfd, read_buf, 8);
    assert0(pthread_mutex_lock(&md->d_mutex));
    void* ret = deque_pop_front(md->d);
    assert0(pthread_mutex_unlock(&md->d_mutex));
    return ret;
}

void* mt_deque_pop_back(mt_deque* md)
{
    uint8_t read_buf[8];
    read(md->push_eventfd, read_buf, 8);
    assert0(pthread_mutex_lock(&md->d_mutex));
    void* ret = deque_pop_back(md->d);
    assert0(pthread_mutex_unlock(&md->d_mutex));
    return ret;
}

bool mt_deque_try_pop_front(mt_deque* md, void** ret)
{
    if (poll_push_eventfd(md))
    {
        pthread_mutex_lock(&md->d_mutex);
        *ret = deque_pop_front(md->d);
        pthread_mutex_unlock(&md->d_mutex);
        return true;
    }
    else
        return false;
}

bool mt_deque_try_pop_back(mt_deque* md, void** ret)
{
    if (poll_push_eventfd(md))
    {
        pthread_mutex_lock(&md->d_mutex);
        *ret = deque_pop_back(md->d);
        pthread_mutex_unlock(&md->d_mutex);
        return true;
    }
    else
        return false;
}
