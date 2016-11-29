#ifndef __IRC_CORE_H
#define __IRC_CORE_H

#include "message.h"
#include "mt_queue.h"

#include <pthread.h>

/**
 * irc_core is a simple IRC client that keeps the connection alive and handles
 * connection registration, nick selection, and reconnections on ping timeouts.
 * All other interaction with an IRC server is done via the public API.
 *
 * irc_core directly passes all incoming messages to the clients.
 */

/** Server information */
typedef struct
{
    char*   server;
    char*   port;
} irc_core_server;

/**
 * User information used for connection registration. Nicks are tried in order.
 * If at the end of the list and nick is still not available, irc_core adds
 * underscores to the last nick until it finds an available nick.
 */
typedef struct
{
    char*   username;
    char*   hostname;
    char*   servername;
    char*   realname;
    char**  nicks;
    int     num_nicks;
} irc_core_user;

// TODO: This should be kept abstract
/** irc_core messaging interface. */
typedef struct irc_core_
{
    irc_core_server server;
    irc_core_user   user;

    /** The irc_core thread. */
    pthread_t       thr;

    /**
     * New messages of type 'message*' are written by client threads using
     * public API.
     *
     * The irc_core thread reads messages off the chan and updates internal
     * state / writes to socket.
     */
    mt_queue*       api_q;

    /**
     * Incoming messages are written to this queue as 'message*'.
     */
    mt_queue*       incoming_msg_q;
} irc_core;

/**
 * Start an irc_core thread.
 *
 * irc_core copies everything it needs so structs and strings can be freed
 * after this function returns.
 */
irc_core* irc_core_start(irc_core_server* server, irc_core_user* user);

void irc_core_free(irc_core*);

/**
 * Return an incoming IRC message. Blocks until a complete message is read.
 * Thread-safe.
 *
 * NULL is returned when irc_core is terminated.
 */
message* irc_core_get_incoming_msg(irc_core*);

/**
 * Get an eventfd that will be signalled when a new message is ready for
 * reading via 'irc_core_get_incoming_msg()'.
 */
int irc_core_get_incoming_msg_eventfd(irc_core*);

/*
 * IRC messages
 */

void privmsg(irc_core*, char* receiver, char* text);
void join   (irc_core*, char* channel);
void part   (irc_core*, char* channel);
void ping   (irc_core*, char* server);
void quit   (irc_core*, char* quit_message);

#endif
