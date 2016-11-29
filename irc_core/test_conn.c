/// This is a program that connects to an IRC server, and just prints received
/// messages.

#include "src/irc_core.h"

static char SERV[] = "chat.freenode.net";
static char PORT[] = "8001";

int main()
{
    irc_core_server server = { .server = SERV, .port = PORT };
    char* nicks[1] = { "tiny_test" };
    irc_core_user   user   = {
        .username = "username",
        .hostname = "hostname",
        .servername = "servername",
        .realname = "tiny",
        .nicks = nicks,
        .num_nicks = 1
    };

    irc_core* irc = irc_core_start(&server, &user);

    message* msg = NULL;
    while ((msg = irc_core_get_incoming_msg(irc)))
    {
        message_print(msg);
        message_free(msg);
    }

    return 0;
}
