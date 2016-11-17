#include "message.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

message* message_parse(char* msg0, int msg_len)
{
    if (msg_len <= 2 || msg0[msg_len - 2] != '\r' || msg0[msg_len - 1] != '\n')
        return NULL;

    // copy the input
    char* complete_msg = malloc(msg_len);
    strncpy(complete_msg, msg0, msg_len);

    char* msg = complete_msg;

    // drop \r\n suffix
    msg_len -= 2;

    ////////////////////////////////////////////////////////////////////////////
    // Parse prefix
    ////////////////////////////////////////////////////////////////////////////

    str_len prefix = { .str = NULL, .len = -1 };

    if (msg[0] == ':')
    {
        // skip ':'
        ++msg;
        --msg_len;

        for (int i = 0; i < msg_len; ++i)
        {
            if (msg[i] == ' ')
            {
                prefix.str = msg;
                prefix.len = i;

                // skip prefix, including trailing whitespace
                msg += i + 1;
                msg_len -= i + 1;

                break;
            }
        }

        if (!prefix.str)
        {
            free(complete_msg);
            return NULL;
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Parse command
    ////////////////////////////////////////////////////////////////////////////

    str_len command = { .str = NULL, .len = -1 };
    {
        int space_idx = 0;
        while (space_idx < msg_len && msg[space_idx] != ' ')
            ++space_idx;

        if (space_idx != msg_len)
        {
            command.str = msg;
            command.len = space_idx;

            // skip command, including trailing space
            msg += space_idx + 1;
            msg_len -= space_idx + 1;
        }
        else
        {
            free(complete_msg);
            return NULL;
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Parse params
    ////////////////////////////////////////////////////////////////////////////

    int num_params = 0;
    str_len params[100];

    while (msg_len > 0)
    {
        if (msg[0] == ':')
        {
            params[num_params].str = msg;
            params[num_params].len = msg_len;
            ++num_params;
            break;
        }
        else
        {
            int space_idx = 0;
            while (space_idx < msg_len && msg[space_idx] != ' ')
                ++space_idx;

            if (space_idx != 0)
            {
                params[num_params].str = msg;
                params[num_params].len = space_idx;
                ++num_params;

                msg += space_idx + 1;

                // msg_len becomes negative after this line when parsing last
                // param of a msg:
                msg_len -= space_idx + 1;
            }
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Allocate and return the message
    ////////////////////////////////////////////////////////////////////////////

    message* ret = malloc(sizeof(message));
    ret->prefix = prefix;
    ret->command = command;

    str_len* params_ret = malloc(sizeof(str_len) * num_params);
    memcpy(params_ret, params, sizeof(str_len) * num_params);
    ret->params = params_ret;
    ret->num_params = num_params;

    ret->full_msg = complete_msg;

    return ret;
}

void message_free(message* msg)
{
    free(msg->full_msg);
    free(msg->params);
    free(msg);
}

void message_print(message* msg)
{
    printf("=== message =============\n");
    printf("Prefix:  %.*s\n", msg->prefix.len, msg->prefix.str);
    printf("Command: %.*s\n", msg->command.len, msg->command.str);
    printf("Params:\n");
    for (int i = 0; i < msg->num_params; ++i)
    {
        str_len* param = &msg->params[i];
        printf("\t%d: %.*s\n", i, param->len, param->str);
    }
    printf("=========================\n");
}
