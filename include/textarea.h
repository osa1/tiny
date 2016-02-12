#ifndef __TINY_TEXTAREA_H
#define __TINY_TEXTAREA_H

#include <stddef.h>

// Multi-line text field. Used for incoming messages.

struct Line_;

struct TextArea_
{
    // Total lines we currently have.
    int             total_lines;

    // Maximum lines we keep in memory before starting dropping the oldest
    // lines.
    int             max_lines;

    // Add new lines to the end.
    struct Line_*   live_lines;
    struct Line_*   live_lines_end;

    // Rendering related
    int             width;
    int             height;

    // Cursor line. This is decremented as lines dropped from the buffer.
    // NOTE: cursor_line < 0 means cursor is not enabled.
    // (Is this a good idea? Why not use a bool?)
    int     cursor_line;

    // Byte offset in the line. INVARIANT: cursor_byte < line.size.
    int     cursor_byte;
};

typedef struct TextArea_ TextArea;

int textarea_new(TextArea* textarea, int max_lines, int width, int height);

void textarea_clean(TextArea* textarea);

int textarea_add_line(TextArea* textarea, char* line, size_t line_len);

void textarea_draw(TextArea* textarea, int pos_x, int pos_y);

#endif
