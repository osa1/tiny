/// Experimenting with the main loop. This version doesn't use threads, relies
/// on poll() to stay responsive.

#include <errno.h>
#include <netdb.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/timerfd.h>
#include <sys/types.h>
#include <unistd.h>

#include "src/msg_buf.h"
#include "src/mt_deque.h"

static char SERV[] = "chat.freenode.net";
static char PORT[] = "8001";

static const int PING_INTERVAL = 60; // in seconds

int start_conn();

typedef struct mainloop_state_
{
    /// Socket connected to a server.
    int         sock;

    /// Channel for API calls. New messages of type 'irc_msg' are added by
    /// public API calls.
    mt_deque*   api_q;

    /// Outgoing message buffer.
    bytebuf     outgoing_buf;

    /// Incoming message buffer.
    msg_buf     incoming_buf;

    /// A 'timerfd' for disconnect events.
    int         disconnect_timer;

    /// true -> PING message was sent after a disconnect timeout.
    bool        disconnect_ping;

    /// To be used in poll(). { sock, api_q, timerfd }
    struct      pollfd poll_fds[3];
} mainloop_state;

void run(mainloop_state*);

int main()
{
    mainloop_state state;
    state.sock = start_conn();
    state.api_q = mt_deque_new(1);
    bytebuf_init(&state.outgoing_buf, 4096);
    msg_buf_init(&state.incoming_buf);

    // We set the timerfd to non-blocking mode, to be able to reset it after
    // poll() but before reading the timerfd.
    state.disconnect_timer = timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK);
    state.disconnect_ping = false;

    state.poll_fds[0].fd = state.sock;
    state.poll_fds[0].events = POLLIN | POLLOUT;
    state.poll_fds[1].fd = mt_deque_get_push_eventfd(state.api_q);
    state.poll_fds[1].events = POLLIN;
    state.poll_fds[2].fd = state.disconnect_timer;
    state.poll_fds[2].events = POLLIN;

    // Fill the outgoing msg buffer with registration messages
    {
        char* nick_msg = "NICK tiny_test\r\n";
        char* user_msg = "USER tiny tiny tiny tiny\r\n";
        bytebuf_push_bytes(&state.outgoing_buf, (uint8_t*)nick_msg, strlen(nick_msg));
        bytebuf_push_bytes(&state.outgoing_buf, (uint8_t*)user_msg, strlen(user_msg));
    }

    run(&state);

    close(state.sock);
    mt_deque_free(state.api_q);
    bytebuf_destroy(&state.outgoing_buf);
    msg_buf_destroy(&state.incoming_buf);

    return 0;
}

void handle_incoming_msgs(msg_buf*);
void send_pending_msgs(bytebuf*, int sock);
void handle_api_calls(mt_deque* api_q, bytebuf* outgoing_q);

void run(mainloop_state* state)
{
    for (;;)
    {
        fflush(stdout); // to be able to tee

        int poll_ret = poll(state->poll_fds, 3, -1); // TODO: This doesn't check timerfd
        if (poll_ret == -1)
        {
            fprintf(stderr, "poll() failed: %s\n", strerror(errno));
            exit(1);
        }

        // Check socket input //////////////////////////////////////////////////
        if (state->poll_fds[0].revents & POLLIN)
        {
            printf("socket input\n");
            if (msg_buf_append_filedes(&state->incoming_buf, state->sock) == 0)
            {
                printf("Server closed connection.\n");
                return;
            }

            handle_incoming_msgs(&state->incoming_buf);

            // reset the disconnect timer
            state->disconnect_ping = false;
            struct itimerspec timer_spec =
                { .it_interval = { 0 }, .it_value = { .tv_sec = PING_INTERVAL, .tv_nsec = 0 } };
            timerfd_settime(state->disconnect_timer,
                            0 /* relative timer */,
                            &timer_spec,
                            NULL);
        }

        // Check socket output /////////////////////////////////////////////////
        if (state->poll_fds[0].revents & POLLOUT)
        {
            printf("socket output\n");
            send_pending_msgs(&state->outgoing_buf, state->sock);

            // Only keep checking POLLOUT if we have more to send
            if (state->outgoing_buf.len == 0)
                state->poll_fds[0].events &= ~POLLOUT;
        }

        // Check API calls /////////////////////////////////////////////////////
        if (state->poll_fds[1].revents & POLLIN)
        {
            printf("api call\n");
            irc_msg* irc_msg0 = mt_deque_pop_front(state->api_q);
            irc_msg* irc_msg = irc_msg0;
            while (irc_msg != NULL)
            {
                bytebuf_push_bytes(&state->outgoing_buf, irc_msg->contents, irc_msg->len);
                state->poll_fds[0].events |= POLLOUT;
                irc_msg = irc_msg->next;
            }
            if (irc_msg0 != NULL)
                irc_msg_free(irc_msg0);
        }

        // Check disconnect timer //////////////////////////////////////////////
        if (state->poll_fds[2].revents & POLLIN)
        {
            uint8_t read_buf[8];
            int read_ret = read(state->disconnect_timer, read_buf, 8);
            if (read_ret == 8 && state->disconnect_ping)
            {
                printf("Disconnected.\n");
                return;
            }
            else if (read_ret == 8)
            {
                printf("Sending ping msg...\n");
                char ping_msg[100];
                snprintf(ping_msg, 100, "PING %s\r\n", SERV);
                bytebuf_push_bytes(&state->outgoing_buf, (uint8_t*)ping_msg, strlen(ping_msg));
                state->poll_fds[0].events |= POLLOUT;
                state->disconnect_ping = true;
            }
        }
    }
}

void handle_incoming_msgs(msg_buf* msg_buf)
{
    irc_msg* msgs0 = msg_buf_extract_msgs(msg_buf);
    irc_msg* msgs = msgs0;
    while (msgs != NULL)
    {
        printf("msg: %s\n", msgs->contents);
        msgs = msgs->next;
    }
    if (msgs0 != NULL)
        irc_msg_free(msgs0);
}

void send_pending_msgs(bytebuf* buf, int sock)
{
    int send_ret = send(sock, buf->buf, buf->len, 0);
    if (send_ret < 0)
    {
        fprintf(stderr, "send() failed: %s\n", strerror(errno));
        exit(1);
    }
    else
        bytebuf_drop(buf, send_ret);
}

int start_conn()
{
    int status;
    struct addrinfo hints;
    struct addrinfo *servinfo;

    memset(&hints, 0, sizeof(hints));
    hints.ai_family   = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    hints.ai_flags    = AI_PASSIVE;

    printf("getaddrinfo\n");
    if ((status = getaddrinfo(SERV, PORT, &hints, &servinfo)) != 0)
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

    printf("Connected to %s:%s\n", SERV, PORT);
    freeaddrinfo(servinfo);

    return sock;
}
