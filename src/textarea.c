#include "textarea.h"

#include <assert.h>
#include <stdlib.h>
#include <string.h>

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
    {
        return NULL;
    }

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
        // TODO: Can we use realloc here?

        // Allocate a bigger buffer
        char* new_buffer = malloc(contents_len + 1);
        if (new_buffer == NULL)
        {
            return 1;
        }

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
    textarea->free_lines = NULL;
    textarea->width = width;
    textarea->height = height;

    // Initially the cursor is disabled.
    textarea->cursor_line = -1;
    textarea->cursor_byte = 0;

    return 0;
}

void textarea_free(TextArea* textarea)
{
    // TODO
}

int textarea_add_line(TextArea* textarea, char* line, size_t line_len)
{
    assert(textarea->total_lines < textarea->max_lines);

    // Is there a free line buffer we can use?
    if (textarea->free_lines != NULL)
    {
        // Grab the line
        Line* fresh_line = textarea->free_lines;
        textarea->free_lines = fresh_line->next;

        // Copy contents, extend the buffer if necessary
        if (line_add_line(fresh_line, line, line_len))
        {
            return 1;
        }

        // Add line to the list
        if (textarea->total_lines == 0)
        {
            fresh_line->prev = NULL;
            fresh_line->next = NULL;
            textarea->live_lines = fresh_line;
            textarea->live_lines_end = fresh_line;
        }
        else
        {
            fresh_line->prev = textarea->live_lines_end;
            fresh_line->next = NULL;
            textarea->live_lines_end = fresh_line;
        }

        textarea->total_lines++;

        return 0;
    }
    else if (textarea->total_lines == textarea->max_lines)
    {
        // Re-use the oldest line
        Line* line_to_reuse = textarea->live_lines;

        // A sanity check: This can't be happening as max_lives can't be 0
        assert(line_to_reuse != NULL);

        // A sanity check: We shouldn't allocate more than max_lines lines.
        assert(textarea->free_lines == NULL);

        textarea->live_lines = line_to_reuse->next;

        // Re-use the line
        if (line_add_line(line_to_reuse, line, line_len))
        {
            // TODO: errors
        }

        line_to_reuse->prev = textarea->live_lines_end;
        line_to_reuse->next = NULL;
        textarea->live_lines_end = line_to_reuse;

        return 0;
    }
    else
    {
        // Allocate a new line.
        Line* new_line = line_new();
        line_add_line(new_line, line, line_len);
        new_line->prev = textarea->live_lines_end;
        textarea->live_lines_end = new_line;
        textarea->total_lines++;

        return 0;
    }
}

void textarea_draw(TextArea* textarea, int pos_x, int pos_y)
{

}
