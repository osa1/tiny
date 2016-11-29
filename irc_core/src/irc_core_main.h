#ifndef __IRC_CORE_MAIN_H
#define __IRC_CORE_MAIN_H

/**
 * The main loop for irc_core. Briefly:
 *
 * - Connects to the server.
 *
 * - Handles the login sequence (NICK and USER messages, finding a nick if nick
 *   is not available).
 *
 * - Keeps the connection alive by sending PING messages on inactivity and
 *   reconnecting on ping timeout.
 */
void* irc_core_main(void*);

#endif
