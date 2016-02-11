#include "textfield.h"

#include "settings.h"

#include <assert.h>
#include <stdlib.h>
#include <string.h>

#include <ncurses.h>

int textfield_new(TextField* textfield, int buffer_len, int width)
{
    // Add one for null termination.
    char* buffer = malloc(buffer_len + 1);
    memset(buffer, 0, buffer_len + 1);

    if (buffer == NULL)
    {
        return 1;
    }

    // Note that we don't take null termination into account here, the byte at
    // the cursor should always be safe to modify.
    textfield->buffer_len = buffer_len;

    textfield->buffer = buffer;
    textfield->width = width;
    textfield->cursor = 0;

    return 0;
}

void textfield_keypressed(TextField* textfield, int key)
{
    assert( textfield->cursor >= 0 &&
            textfield->cursor <= textfield->buffer_len );

    if (key == KEY_BACKSPACE)
    {
        if (textfield->cursor > 0)
        {
            textfield->cursor -= 1;
        }

        textfield->buffer[ textfield->cursor ] = '\0';
    }
    else if (textfield->cursor < textfield->buffer_len)
    {
        textfield->buffer[ textfield->cursor ] = key;
        textfield->cursor += 1;
    }
}

void textfield_draw(TextField* textfield, int pos_x, int pos_y)
{
    mvwaddch( stdscr, pos_y, pos_x + 0, '>' );
    mvwaddch( stdscr, pos_y, pos_x + 1, ' ' );

    // Internally we make sure the buffer is null terminated, so this is OK.
    mvwaddstr( stdscr, pos_y, pos_x + 2, textfield->buffer );

    // Draw the cursor
    int len = strlen( textfield->buffer ) + 2;

    // TODO: Make sure we don't go out of bounds.

    attron( COLOR_PAIR( COLOR_CURSOR ) );
    mvwaddch( stdscr, pos_y, pos_x + len, ' ' );
    attroff( COLOR_PAIR( COLOR_CURSOR ) );

    // clear rest of the line
    while ( ++len < textfield->width + 2 )
    {
        mvwaddch( stdscr, pos_y, pos_x + len, ' ' );
    }
}
