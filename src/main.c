#include <assert.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <errno.h>
#include <netdb.h>
#include <signal.h>
#include <sys/select.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <unistd.h>

#include <ncurses.h>

#include "settings.h"
#include "textarea.h"
#include "textfield.h"

// According to rfc2812, IRC messages can't exceed 512 characters - and this
// includes \r\n, which follows every IRC message.
#define RECV_BUF_SIZE 512

static char recv_buf[ RECV_BUF_SIZE ] = {0};

////////////////////////////////////////////////////////////////////////////////

void mainloop();
void abort_msg(const char* fmt, ...);
int clear_cr_nl();

////////////////////////////////////////////////////////////////////////////////

static volatile sig_atomic_t got_sigwinch = 0;

void sigwinch_handler(int sig)
{
    got_sigwinch = 1;
}

////////////////////////////////////////////////////////////////////////////////

int main()
{
    struct sigaction sa;
    sa.sa_handler = sigwinch_handler;
    sa.sa_flags = SA_RESTART;
    sigemptyset(&sa.sa_mask);

    if (sigaction(SIGWINCH, &sa, NULL) == -1)
    {
        printf("Can't register SIGWINCH action.\n");
        exit(1);
    }

    initscr();
    noecho();
    keypad( stdscr, TRUE );
    curs_set( 0 );
    raw();

    start_color();
    init_pair( COLOR_CURSOR, COLOR_WHITE, COLOR_GREEN );

    mainloop();

    endwin();

    return 0;
}

void mainloop()
{
    abort_msg("Connecting..." );
    wrefresh( stdscr );

    struct addrinfo hints;
    memset(&hints, 0, sizeof(struct addrinfo));

    struct addrinfo* res;

    hints.ai_family = AF_INET;
    hints.ai_socktype = SOCK_STREAM;

    if ( getaddrinfo( "chat.freenode.org", "6665", &hints, &res ) )
    {
        abort_msg("getaddrinfo(): %s", strerror(errno) );
        wrefresh( stdscr );
        return;
    }

    int sock = socket( AF_INET, SOCK_STREAM, 0 );

    if ( connect( sock, res->ai_addr, res->ai_addrlen ) )
    {
        abort_msg("connect(): %s", strerror(errno) );
        wrefresh( stdscr );
        return;
    }

    abort_msg("seems like worked" );
    wrefresh( stdscr );

    fd_set rfds;
    // Watch stdin (fd 0) to see when it has input.
    FD_ZERO( &rfds );
    FD_SET( 0, &rfds );
    FD_SET( sock, &rfds );
    int fdmax = sock;

    TextField input_field;
    textfield_new(&input_field, RECV_BUF_SIZE, COLS);

    TextArea msg_area;
    textarea_new(&msg_area, 100, COLS, LINES - 2);

    while ( true )
    {
        fd_set rfds_ = rfds;
        if ( select( fdmax + 1, &rfds_, NULL, NULL, NULL ) == -1 )
        {
            if (errno == ERESTART)
            {
                // probably SIGWINCH during select()
                if (got_sigwinch == 1)
                {
                    got_sigwinch = 0;
                    endwin();
                    refresh();

                    input_field.width = COLS;
                    msg_area.height = LINES - 2;
                    msg_area.width = COLS;

                    continue;
                }
                else
                {
                    // TODO: report this
                    break;
                }
            }
        }
        else if ( FD_ISSET( 0, &rfds_ ) )
        {
            // stdin is ready
            int ch = getch();
            KeypressRet ret = textfield_keypressed(&input_field, ch);
            if (ret == SHIP_IT)
            {
                // Ops.. This won't work. We need \r\n.
                int msg_len = strlen(input_field.buffer);

                // We need \r\n suffix before sending the message.
                // FIXME: This is not how you do it though.
                input_field.buffer[msg_len    ] = '\r';
                input_field.buffer[msg_len + 1] = '\n';
                send(sock, input_field.buffer, msg_len + 2, 0);
                textarea_add_line(&msg_area, input_field.buffer, msg_len);
                textfield_reset(&input_field);
            }
            else if (ret == ABORT)
            {
                break;
            }
        }
        else if ( FD_ISSET( sock, &rfds_ ) )
        {
            // socket is ready
            int recv_ret = recv( sock, recv_buf, RECV_BUF_SIZE, 0 );
            if ( recv_ret == -1 )
            {
                abort_msg("recv(): %s", strerror(errno) );
            }
            else if ( recv_ret == 0 )
            {
                abort_msg("connection closed" );
                break;
            }
            else
            {
                abort_msg("recv() got partial msg of len %d",
                          recv_ret);

                int cursor_inc = clear_cr_nl();
                textarea_add_line(&msg_area, recv_buf, cursor_inc);
            }
        }

        // For now draw everyting from scratch on any event
        wclear(stdscr);
        textfield_draw(&input_field, 0, LINES - 2);
        textarea_draw(&msg_area, 0, 0);
        wrefresh(stdscr);
    }

    close(sock);
}

// This is used for two things:
//
// * We don't want to print \r\n as it confuses ncurses and/or terminals
//   (cursor moves to new line etc.)
//
// * We put the null terminator for printing.
//
// * It returns length of the string, so it can be used for incrementing the
//   cursor etc.
int clear_cr_nl()
{
    for ( int i = 0; i < RECV_BUF_SIZE - 1; ++i )
    {
        if ( recv_buf[ i ] == '\r' )
        {
            recv_buf[ i     ] = 0;
            recv_buf[ i + 1 ] = 0;
            return i;
        }
        else if ( recv_buf[ i ] == '\0' )
        {
            return i;
        }
    }

    return 0;
}

void abort_msg(const char* fmt, ... )
{
    va_list argptr;
    va_start( argptr, fmt );

    // Clear the line
    for ( int i = 0; i < COLS; i++ )
    {
        mvwaddch( stdscr, LINES - 1, i, ' ' );
    }

    wmove( stdscr, LINES - 1, 0 );
    vwprintw( stdscr, fmt, argptr );

    va_end( argptr );
}
