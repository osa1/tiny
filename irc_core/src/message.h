#ifndef __IRC_CORE_MESSAGE_H
#define __IRC_CORE_MESSAGE_H

#include <stdint.h>

/**
 * A string type with length information. The string DOES NOT terminate with a
 * null byte.
 */
typedef struct str_len_
{
    char* str;
    int   len;
} str_len;

typedef struct message_
{
    /**
     * <prefix> in RHC 1459. Does not include ':' and trailing space.
     */
    str_len     prefix;

    str_len     command;

    str_len*    params;
    int         num_params;

    /**
     * Internal copy of the original message. Other string pointers point to
     * this string.
     */
    char*       full_msg;
} message;

message* message_parse(char* msg, int msg_len);

void message_free(message*);

/** For debugging purposes. */
void message_print(message*);

void message_write(message*, uint8_t*);

#endif
