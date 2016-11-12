#include <poll.h>
#include <pthread.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>

#include "mt_deque.h"

#define MAX_IRC_THREADS 100

static pthread_t threads[MAX_IRC_THREADS];

typedef struct irc_core_
{
    int sock;
    mt_deque* msg_queue;
} irc_core;

void* irc_core_main(irc_core* core)
{
    (void)core;

    // - Read from the socket, handle complete messages are '\r\n' delimiters are read.
    // - Send messages from the outgoing message queue as new messages are added.
    //
    // So we need to poll():
    //
    // - Input socket.
    // - Output socket.
    // - Outgoing message queue.

    struct pollfd fds[2] =
        { { .fd = core->sock, .events = POLLIN | POLLOUT }
        , { .fd = mt_deque_get_push_eventfd(core->msg_queue), .events = POLLIN }
        };

    // main loop
    for (;;)
    {
        poll(fds, 2, -1);

    }

    return NULL;
}

// sock: Non-blocking socket connected to an IRC server.
irc_core* irc_core_start(int sock)
{
    // Find empty spot in 'threads'
    int thread_slot = 0;
    while (thread_slot < MAX_IRC_THREADS && threads[thread_slot])
        ++thread_slot;

    irc_core* core = malloc(sizeof(irc_core));
    core->sock = sock;
    core->msg_queue = mt_deque_new(10);

    pthread_create(&threads[thread_slot], NULL, (void*(*)(void*))irc_core_main, core);

    return core;
}
