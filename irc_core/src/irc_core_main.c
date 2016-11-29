#include "irc_core.h"
#include "irc_core_main.h"
#include "msg_buf.h"

#include <assert.h>
#include <errno.h>
#include <netdb.h>
#include <poll.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/timerfd.h>
#include <sys/types.h>
#include <unistd.h>

typedef struct mainloop_state_
{
    /**
     * Socket connected to the server. We close() this one and open() a new
     * one in case of a disconnect.
     */
    int             sock;

    /**
     * Outgoing message buffer. Bytes are collected here until the socket
     * becomes ready for sending.
     */
    bytebuf         outgoing_buf;

    /** Incoming message buffer. Partial messages are collected here. */
    msg_buf         incoming_buf;

    /** A 'timerfd' for disconnect events. */
    int             disconnect_timer;

    /** true -> PING message was sent after a disconnect timeout. */
    bool            disconnect_ping;

    /** To be used in poll(). { sock, api_q, timerfd } */
    struct pollfd   poll_fds[3];

    /**
     * Currently used nick. If this is larger than 'irc_core_user.num_nicks',
     * we add 'irc_core_user.num_nicks - current_nick' underscores to the last
     * nick in 'irc_core_user.nicks'.
     *
     * -1 means nick was choosen with `irc_core_nick()`.
     */
    int             current_nick;
} mainloop_state;

int irc_core_connect(irc_core_server*);

typedef enum
{
    QUIT,
    DISCONNECT,
} loop_ret;

loop_ret loop(irc_core*, mainloop_state*);

void* irc_core_main(void* irc0)
{
    irc_core* irc = irc0;
    mainloop_state state;

    for (;;)
    {
        // Initialize state
        bytebuf_init(&state.outgoing_buf, 4096);
        msg_buf_init(&state.incoming_buf);
        state.disconnect_ping = false;
        state.current_nick = 0;

        // Connect
        state.sock = irc_core_connect(&irc->server);
        state.disconnect_timer = timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK);
        assert(state.disconnect_timer != -1);

        state.poll_fds[0].fd = state.sock;
        state.poll_fds[0].events = POLLIN; // only activate POLLOUT when outgoing_buf is not empty
        state.poll_fds[1].fd = mt_queue_get_eventfd(irc->api_q);
        state.poll_fds[1].events = POLLIN;
        state.poll_fds[2].fd = state.disconnect_timer;
        state.poll_fds[2].events = POLLIN;

        // Loop until a QUIT message or disconnect
        loop_ret ret = loop(irc, &state);
        close(state.sock);
        bytebuf_destroy(&state.outgoing_buf);
        msg_buf_destroy(&state.incoming_buf);
        close(state.disconnect_timer);
        if (ret == QUIT)
        {
            // TODO: cleanup state
            printf("QUIT\n");
            mt_queue_push(irc->incoming_msg_q, NULL);
            break;
        }
        else if (ret == DISCONNECT) {}
        else
            assert(false);
    }

    return NULL;
}

int irc_core_connect(irc_core_server* server)
{
    int status;
    struct addrinfo hints;
    struct addrinfo *servinfo;

    memset(&hints, 0, sizeof(hints));
    hints.ai_family   = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    hints.ai_flags    = AI_PASSIVE;

    printf("getaddrinfo\n");
    if ((status = getaddrinfo(server->server, server->port, &hints, &servinfo)) != 0)
    {
        fprintf(stderr, "getaddrinfo error: %s\n", gai_strerror(status));
        exit(1);
    }

    int sock = socket(servinfo->ai_family, SOCK_STREAM, 0);
    printf("connect\n");
    if ((status = connect(sock, servinfo->ai_addr, servinfo->ai_addrlen)) != 0)
    {
        fprintf(stderr, "connect() error: %s\n", gai_strerror(status));
        exit(1);
    }

    printf("Connected to %s:%s\n", server->server, server->port);
    freeaddrinfo(servinfo);

    return sock;
}

loop_ret loop(irc_core* irc, mainloop_state* state)
{
    int poll_ret;


    state->disconnect_ping = false;
    struct itimerspec timer_spec =
        { .it_interval = { 0 }, .it_value = { .tv_sec = 60 /* in seconds */, .tv_nsec = 0 } };
    timerfd_settime(state->disconnect_timer,
                    0 /* relative timer */,
                    &timer_spec,
                    NULL);

    for (;;)
    {

        if (state->outgoing_buf.len != 0)
            state->poll_fds[0].events = POLLIN | POLLOUT;
        else
            state->poll_fds[0].events = POLLIN;

        printf("poll()\n");
        if ((poll_ret = poll(state->poll_fds, 3, -1 /* block indefinitely */)) > 0)
        {
            // check recv()
            if (state->poll_fds[0].revents & POLLIN)
            {
                printf("recv()\n");
                int bytes_read = msg_buf_append_fd(&state->incoming_buf, state->sock);
                // TODO: check bytes_read. 0 means socket was closed at the other end.
                irc_msg* msgs0 = msg_buf_extract_msgs(&state->incoming_buf);
                irc_msg* msgs = msgs0;
                while (msgs)
                {
                    message* msg = message_parse((char*)msgs->contents, msgs->len);
                    printf("pushing msg:\n");
                    message_print(msg); // FIXME: There's a bug here when we can't parse a message (segfault)
                    mt_queue_push(irc->incoming_msg_q, msg);
                    msgs = msgs->next;
                }
                irc_msg_free(msgs0);
            }
            // check send()
            if (state->poll_fds[0].revents & POLLOUT)
            {
                printf("send()\n");
                bytebuf_write_fd(&state->outgoing_buf, state->sock);
            }
            // check timer
            if (state->poll_fds[1].revents & POLLIN)
            {
                printf("timerfd()\n");
                if (state->disconnect_ping)
                    return DISCONNECT;
                else
                {
                    ping(irc, ""); // FIXME: server
                    state->disconnect_ping = true;
                }
            }
            // Check api_q
            if (state->poll_fds[2].revents & POLLIN)
            {
                printf("api_q()\n");
                message* msg = mt_queue_pop(irc->api_q);
                bytebuf_reserve(&state->outgoing_buf, 512); // max size of an IRC message
                message_write(msg, state->outgoing_buf.buf + state->outgoing_buf.len);
                // TODO: If message is a NICK message we should update our nick state
            }
        }
    }

    printf("poll ret: %d\n", poll_ret);
}
