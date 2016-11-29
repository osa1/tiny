#include "irc_core.h"
#include "irc_core_main.h"
#include "msg_buf.h"

#include <stdbool.h>
#include <stdlib.h>
#include <string.h>

irc_core* irc_core_start(irc_core_server* server, irc_core_user* user)
{
    irc_core* irc = malloc(sizeof(irc_core));

    irc->server.server      = strdup(server->server);
    irc->server.port        = strdup(server->port);
    irc->user.username      = strdup(user->username);
    irc->user.hostname      = strdup(user->hostname);
    irc->user.servername    = strdup(user->servername);
    irc->user.realname      = strdup(user->realname);

    irc->user.num_nicks = user->num_nicks;
    irc->user.nicks     = malloc(sizeof(char*) * user->num_nicks);
    for (int i = 0; i < user->num_nicks; ++i)
        *(irc->user.nicks + i) = strdup(*(user->nicks + i));

    irc->api_q = mt_queue_new(10);
    irc->incoming_msg_q = mt_queue_new(10);

    pthread_create(&irc->thr, 0, irc_core_main, irc);

    return irc;
}

void irc_core_free(irc_core* irc)
{
    (void)irc;
}

message* irc_core_get_incoming_msg(irc_core* irc)
{
    return mt_queue_pop(irc->incoming_msg_q);
}

int irc_core_get_incoming_msg_eventfd(irc_core* irc)
{
    return mt_queue_get_eventfd(irc->incoming_msg_q);
}

/*
 * IRC messages
 */

void ping(irc_core* irc, char* server)
{
    message* msg = malloc(sizeof(message));
    // TODO: fill in the msg
    mt_queue_push(irc->api_q, msg);
}
