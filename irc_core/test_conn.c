/// This is a program that connects to an IRC server, and just prints received
/// messages. DOES NOT keep the connection alive.

#include <netdb.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <unistd.h>

#include "src/msg_buf.h"

static char SERV[] = "chat.freenode.net";
static char PORT[] = "8001";

int main()
{
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
        fprintf(stderr, "conenct() error: %s\n", gai_strerror(status));
        exit(1);
    }

    printf("Connected to %s:%s\n", SERV, PORT);

    msg_buf buf;
    msg_buf_init(&buf);

    struct pollfd poll_fds[1] = { { .fd = sock, .events = POLLIN } };

    for (;;)
    {
        poll_fds[0].revents = 0;
        poll(poll_fds, 1, -1);
        printf("Reading from socket.\n");
        msg_buf_append_filedes(&buf, sock);
        printf("Extracting messages...\n");
        irc_msg* msgs0 = msg_buf_extract_msgs(&buf);
        irc_msg* msgs = msgs0;
        while (msgs != NULL)
        {
            printf("msg: %s\n", msgs->contents);
            msgs = msgs->next;
        }
        if (msgs0 != NULL)
            irc_msg_free(msgs0);
    }

    close(sock);
    freeaddrinfo(servinfo);
}


