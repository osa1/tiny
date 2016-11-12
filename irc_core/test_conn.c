/// This is a program that connects to an IRC server, and just prints received
/// messages. DOES NOT keep the connection alive.

#include <assert.h>
#include <netdb.h>
#include <poll.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <unistd.h>

#include "src/msg_buf.h"
#include "src/mt_deque.h"

static char SERV[] = "chat.freenode.net";
static char PORT[] = "8001";

static void* conn_thread_fn(void* st);
static void* msg_thread_fn(void* st);

int main()
{
    pthread_t conn_thread, msg_thread;
    int pt_ret;
    mt_deque* msg_queue = mt_deque_new(10);

    pt_ret = pthread_create(&conn_thread, NULL, conn_thread_fn, msg_queue);
    assert(pt_ret == 0);

    pt_ret = pthread_create(&msg_thread, NULL, msg_thread_fn, msg_queue);
    assert(pt_ret == 0);

    pt_ret = pthread_join(conn_thread, NULL);
    assert(pt_ret == 0);
    pt_ret = pthread_join(msg_thread, NULL);
    assert(pt_ret == 0);

    mt_deque_free(msg_queue);
    return 0;
}

static void* conn_thread_fn(void* st)
{
    mt_deque* msg_q = st;

    int status;
    struct addrinfo hints;
    struct addrinfo *servinfo;

    memset(&hints, 0, sizeof(hints));
    hints.ai_family   = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    hints.ai_flags    = AI_PASSIVE;

    if ((status = getaddrinfo(SERV, PORT, &hints, &servinfo)) != 0)
    {
        fprintf(stderr, "getaddrinfo error: %s\n", gai_strerror(status));
        exit(1);
    }

    int sock = socket(servinfo->ai_family, SOCK_STREAM, 0);
    if ((status = connect(sock, servinfo->ai_addr, servinfo->ai_addrlen)) != 0)
    {
        fprintf(stderr, "connect() error: %s\n", gai_strerror(status));
        exit(1);
    }

    printf("Connected to %s:%s\n", SERV, PORT);

    msg_buf buf;
    msg_buf_init(&buf);

    struct pollfd poll_fds[1] = { { .fd = sock, .events = POLLIN } };

    for (;;)
    {
        poll_fds[0].revents = 0;
        if (poll(poll_fds, 1, 5000) == 0)
        {
            printf("poll() timed out. Aborting.\n");
            mt_deque_push_front(msg_q, NULL);
            break;
        }

        printf("Reading from socket.\n");
        msg_buf_append_filedes(&buf, sock);
        printf("Extracting messages...\n");
        irc_msg* msgs = msg_buf_extract_msgs(&buf);
        mt_deque_push_front(msg_q, msgs);
    }

    msg_buf_destroy(&buf);
    close(sock);
    freeaddrinfo(servinfo);
    return NULL;
}

static void* msg_thread_fn(void* st)
{
    mt_deque* msg_q = st;

    irc_msg* msgs0 = NULL;
    while ((msgs0 = mt_deque_pop_back(msg_q)) != NULL)
    {
        irc_msg* msgs = msgs0;
        while (msgs != NULL)
        {
            printf("msg: %s\n", msgs->contents);
            msgs = msgs->next;
        }
        if (msgs0 != NULL)
            irc_msg_free(msgs0);
    }

    return NULL;
}
