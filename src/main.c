#include <assert.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

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

////////////////////////////////////////////////////////////////////////////////

#define MIN(x, y) (x < y ? x : y)

////////////////////////////////////////////////////////////////////////////////

#define COLOR_CURSOR 1

////////////////////////////////////////////////////////////////////////////////

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
void draw_input_line( Layout*, char[ RECV_BUF_SIZE ] );
void handle_input( int, char[ RECV_BUF_SIZE ], int* cursor );

int main()
{
    initscr();
    noecho();
    keypad( stdscr, TRUE );
    curs_set( 0 );
    raw();

    start_color();
    init_pair( COLOR_CURSOR, COLOR_WHITE, COLOR_GREEN );

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
    abort_msg( &layout, "Connecting..." );
    wrefresh( stdscr );

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
            handle_input( ch, input_buf, &input_cursor );
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

        draw_input_line( &layout, input_buf );

        wrefresh( stdscr );
    }
}

void handle_input( int ch, char input[ RECV_BUF_SIZE ], int* cursor )
{
    // FIXME: We may miss the array if cursor goes too far.
    // FIXME: Actually, we should have the 510-char limit for input. We can
    //        just send messages in multiple send()s.
    // FIXME: Actually, 510-char limit is already broken. Messages will have
    //        PRIVMSG etc. prefix so we actually have less space.
    assert( *cursor >= 0 && *cursor <= RECV_BUF_SIZE );

    if ( ch == KEY_BACKSPACE )
    {

        if ( *cursor > 0 )
        {
            *cursor -= 1;
        }

        input[ *cursor ] = '\0';
    }
    else if ( *cursor + 1 < RECV_BUF_SIZE )
    {
        input[ *cursor ] = ch;
        *cursor = *cursor + 1;
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

void draw_input_line( Layout* layout, char input[ RECV_BUF_SIZE ] )
{
    int row = layout->height - 2;

    mvwaddch( stdscr, row, 0, '>' );
    mvwaddch( stdscr, row, 1, ' ' );

    // input is null terminated, right? RIGHT?
    mvwaddstr( stdscr, row, 2, input );

    // draw cursor
    int len = strlen( input ) + 2;

    attron( COLOR_PAIR( COLOR_CURSOR ) );
    mvwaddch( stdscr, row, len, ' ' );
    attroff( COLOR_PAIR( COLOR_CURSOR ) );

    // clear rest of the line
    while ( ++len < MIN( layout->width, RECV_BUF_SIZE ) + 2 )
    {
        mvwaddch( stdscr, row, len, ' ' );
    }
}

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
}
