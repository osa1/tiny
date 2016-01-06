#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

#include <errno.h>
#include <netdb.h>
#include <sys/select.h>
#include <sys/socket.h>
#include <sys/types.h>

#include <ncurses.h>

// According to rfc2812, IRC messages can't exceed 512 characters - and this
// includes \r\n, which follows every IRC message.
#define RECV_BUF_SIZE 512

static char recv_buf[ RECV_BUF_SIZE ] = {0};

typedef struct _layout
{
    int32_t width;
    int32_t height;

    int32_t cursor_x;
    int32_t cursor_y;
} Layout;

void mainloop( Layout );
void abort_msg( Layout*, const char* fmt, ... );
int clear_cr_nl();
void draw_input_line( Layout* );

int main()
{
    initscr();
    noecho();
    keypad( stdscr, TRUE );
    curs_set( 0 );
    raw();

    int scr_height, scr_width;
    getmaxyx( stdscr, scr_height, scr_width );

    Layout layout = {scr_width, scr_height, 0, 0};

    mainloop( layout );

    // printw("hello world");
    // refresh();
    // getch();

    getch();
    endwin();

    return 0;
}

void mainloop( Layout layout )
{
    struct addrinfo hints = {};
    struct addrinfo* res;

    hints.ai_family = AF_INET;
    hints.ai_socktype = SOCK_STREAM;

    if ( getaddrinfo( "chat.freenode.org", "6665", &hints, &res ) )
    {
        abort_msg( &layout, "getaddrinfo failed: %d", errno );
        wrefresh( stdscr );
        return;
    }

    int sock = socket( AF_INET, SOCK_STREAM, 0 );

    if ( connect( sock, res->ai_addr, res->ai_addrlen ) )
    {
        abort_msg( &layout, "connect() failed: %d", errno );
        wrefresh( stdscr );
        return;
    }

    abort_msg( &layout, "seems like worked %d", 10 );
    wrefresh( stdscr );

    fd_set rfds;
    //  Watch stdin (fd 0) to see when it has input.
    FD_ZERO( &rfds );
    FD_SET( 0, &rfds );
    FD_SET( sock, &rfds );
    int fdmax = sock;

    char input_buf[ RECV_BUF_SIZE ] = {0};
    int input_cursor = 0;

    while ( true )
    {
        fd_set rfds_ = rfds;
        if ( select( fdmax + 1, &rfds_, NULL, NULL, NULL ) == -1 )
        {
            abort_msg( &layout, "select() failed" );
            break;
        }

        if ( FD_ISSET( 0, &rfds_ ) )
        {
            // stdin is ready
            abort_msg( &layout, "stdin is ready" );
            int ch = getch();
        }
        else if ( FD_ISSET( sock, &rfds_ ) )
        {
            // socket is ready
            int recv_ret = recv( sock, recv_buf, RECV_BUF_SIZE, 0 );
            if ( recv_ret == -1 )
            {
                abort_msg( &layout, "recv() error: %d", errno );
            }
            else if ( recv_ret == 0 )
            {
                abort_msg( &layout, "connection closed" );
                break;
            }
            else
            {
                abort_msg( &layout, "recv() got partial msg of len %d",
                           recv_ret );

                int cursor_inc = clear_cr_nl();
                mvwprintw( stdscr, layout.cursor_y, layout.cursor_x, recv_buf );
                layout.cursor_x += cursor_inc;

                if ( layout.cursor_x > layout.width )
                {
                    mvwprintw( stdscr, layout.cursor_y + 1, 0,
                               recv_buf + ( layout.cursor_x - layout.width ) );
                    layout.cursor_x = 0;
                    layout.cursor_y += 2;
                }
                else
                {
                    layout.cursor_x = 0;
                    layout.cursor_y += 1;
                }
            }
        }

        draw_input_line( &layout );

        wrefresh( stdscr );
    }
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
    for ( int i = 0; i < RECV_BUF_SIZE; ++i )
    {
        if ( recv_buf[ i ] == '\r' )
        {
            recv_buf[ i ] = 0;
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

void draw_input_line( Layout* layout ) {}

void abort_msg( Layout* layout, const char* fmt, ... )
{
    va_list argptr;
    va_start( argptr, fmt );

    // Clear the line
    for ( int i = 0; i < layout->width; i++ )
    {
        mvwaddch( stdscr, layout->height - 1, i, ' ' );
    }

    wmove( stdscr, layout->height - 1, 0 );
    vwprintw( stdscr, fmt, argptr );

    va_end( argptr );

    wrefresh( stdscr );
}
