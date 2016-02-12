#include "textarea.h"

#include <assert.h>
#include <stdlib.h>
#include <string.h>

#include <ncurses.h>

////////////////////////////////////////////////////////////////////////////////

#define MIN(a, b) ((a) < (b) ? (a) : (b))

////////////////////////////////////////////////////////////////////////////////
// We keep a linked list of lines to be able to easily allocate/deallocate
// lines. Lines are null-terminated to be able to easily render using ncurses
// API.

struct Line_
{
    char*           buffer;
    size_t          buffer_size;
    struct Line_*   next;
    struct Line_*   prev;
};

typedef struct Line_ Line;

Line* line_new()
{
    Line* line = malloc(sizeof(Line));
    if (line == NULL)
        return NULL;

    line->buffer = NULL;
    line->buffer_size = 0;
    line->next = NULL;
    line->prev = NULL;

    return line;
}

// NOTE: Doesn't follow pointers.
void line_free(Line* line)
{
    assert(line != NULL);
    free(line->buffer);
    free(line);
}

int line_add_line(Line* line, char* contents, size_t contents_len)
{
    // Is the buffer big enough? (+1 for null termination)
    if (line->buffer_size < contents_len + 1)
    {
        // Allocate a bigger buffer
        char* new_buffer = malloc(contents_len + 1);
        if (new_buffer == NULL)
            return 1;

        free(line->buffer);
        line->buffer = new_buffer;
        line->buffer_size = contents_len + 1;
    }

    // Copy contents
    memcpy(line->buffer, contents, contents_len);
    line->buffer[ contents_len ] = '\0';
    // TODO: Should we note the line_len here?

    return 0;
}

Line* line_nth_rev(Line* line, int idx)
{
    while (idx > 0 && line != NULL)
    {
        line = line->prev;
        idx--;
    }

    return line;
}

////////////////////////////////////////////////////////////////////////////////

int textarea_new(TextArea* textarea, int max_lines, int width, int height)
{
    if (max_lines == 0)
    {
        return 1;
    }

    textarea->total_lines = 0;
    textarea->max_lines = max_lines;
    textarea->live_lines = NULL;
    textarea->live_lines_end = NULL;
    textarea->width = width;
    textarea->height = height;

    // Initially the cursor is disabled.
    textarea->cursor_line = -1;
    textarea->cursor_byte = 0;

    return 0;
}

void textarea_clean(TextArea* textarea)
{
    Line* line = textarea->live_lines;
    while (line != NULL)
    {
        Line* next_line = line->next;
        line_free(line);
        line = next_line;
    }
}

int textarea_add_line(TextArea* textarea, char* line, size_t line_len)
{
    assert(textarea->total_lines <= textarea->max_lines);

    Line* new_line = NULL;

    if (textarea->total_lines == textarea->max_lines)
    {
        // Re-use the oldest line
        new_line = textarea->live_lines;

        // Sanity check: This can't be happening as max_lives can't be 0
        assert(new_line != NULL);

        textarea->live_lines = new_line->next;
        textarea->live_lines->prev = NULL;
    }
    else
    {
        // Allocate a new line.
        new_line = line_new();
        textarea->total_lines++;
    }

    assert(new_line != NULL);

    // Set the line buffer
    if (line_add_line(new_line, line, line_len))
        return 1;

    if (textarea->live_lines == NULL)
    {
        new_line->prev = NULL;
        new_line->next = NULL;
        textarea->live_lines = new_line;
        textarea->live_lines_end = new_line;
    }
    else
    {
        assert(textarea->live_lines_end != NULL);
        new_line->prev = textarea->live_lines_end;
        new_line->next = NULL;
        textarea->live_lines_end->next = new_line;
        textarea->live_lines_end = new_line;
    }

    return 0;
}

void textarea_draw(TextArea* textarea, int pos_x, int pos_y)
{
    // FIXME: Current assuming one line = one row
    int height          = textarea->height;
    int total_lines     = textarea->total_lines;
    int lines_to_draw   = MIN(height, total_lines);

    Line* line    = line_nth_rev(textarea->live_lines_end, lines_to_draw - 1);
    int line_row  = pos_y + height - lines_to_draw;

    while (line != NULL)
    {
        mvwprintw(stdscr, line_row, pos_x, line->buffer);
        line = line->next;
        line_row++;
    }
}
