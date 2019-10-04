#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdbool.h>
#include <sys/ioctl.h>
#include <sys/time.h>
#include <sys/stat.h>
#include <termios.h>
#include <unistd.h>
#include <wchar.h>

#include "termbox.h"

#include "bytebuffer.inl"
#include "term.inl"

struct cellbuf {
    int width;
    int height;
    struct tb_cell *cells;
};

#define CELL(buf, x, y) (buf)->cells[(y) * (buf)->width + (x)]
#define IS_CURSOR_HIDDEN(cx, cy) (cx == -1 || cy == -1)
#define LAST_COORD_INIT -1

static struct termios orig_tios;

static struct cellbuf back_buffer;
static struct cellbuf front_buffer;
static struct bytebuffer output_buffer;

static int termw = -1;
static int termh = -1;

static int outputmode = TB_OUTPUT_NORMAL;

static int inout;

static int lastx = LAST_COORD_INIT;
static int lasty = LAST_COORD_INIT;
static int cursor_x = -1;
static int cursor_y = -1;

static uint16_t background = TB_DEFAULT;
static uint16_t foreground = TB_DEFAULT;

static void write_cursor(int x, int y);
static void write_sgr(uint16_t fg, uint16_t bg);

static void cellbuf_init(struct cellbuf *buf, int width, int height);
static void cellbuf_resize(struct cellbuf *buf, int width, int height);
static void cellbuf_clear(struct cellbuf *buf);
static void cellbuf_free(struct cellbuf *buf);

static void update_size(void);
static void update_term_size(void);
static void send_attr(uint16_t fg, uint16_t bg);
static void send_char(int x, int y, uint32_t c);
static void send_clear(void);

/* may happen in a different thread */
static volatile int buffer_size_change_request;

/* -------------------------------------------------------- */

int tb_init(void)
{
    inout = open("/dev/tty", O_RDWR);
    if (inout == -1) {
        return TB_EFAILED_TO_OPEN_TTY;
    }

    if (init_term() < 0) {
        close(inout);
        return TB_EUNSUPPORTED_TERMINAL;
    }

    tcgetattr(inout, &orig_tios);

    struct termios tios;
    memcpy(&tios, &orig_tios, sizeof(tios));

    tios.c_iflag &= ~(IGNBRK | BRKINT | PARMRK | ISTRIP
                           | INLCR | IGNCR | ICRNL | IXON);
    tios.c_oflag &= ~OPOST;
    tios.c_lflag &= ~(ECHO | ECHONL | ICANON | ISIG | IEXTEN);
    tios.c_cflag &= ~(CSIZE | PARENB);
    tios.c_cflag |= CS8;
    tios.c_cc[VMIN] = 0;
    tios.c_cc[VTIME] = 0;
    tcsetattr(inout, TCSAFLUSH, &tios);

    bytebuffer_init(&output_buffer, 32 * 1024);

    bytebuffer_puts(&output_buffer, funcs[T_ENTER_CA]);
    bytebuffer_puts(&output_buffer, funcs[T_ENTER_KEYPAD]);
    bytebuffer_puts(&output_buffer, funcs[T_HIDE_CURSOR]);
    bytebuffer_puts(&output_buffer, funcs[T_ENABLE_FOCUS_EVENTS]);
    send_clear();

    update_term_size();
    cellbuf_init(&back_buffer, termw, termh);
    cellbuf_init(&front_buffer, termw, termh);
    cellbuf_clear(&back_buffer);
    cellbuf_clear(&front_buffer);

    return 0;
}

void tb_shutdown(void)
{
    if (termw == -1) {
        fputs("tb_shutdown() should not be called twice.", stderr);
        abort();
    }

    bytebuffer_puts(&output_buffer, funcs[T_SHOW_CURSOR]);
    bytebuffer_puts(&output_buffer, funcs[T_SGR0]);
    bytebuffer_puts(&output_buffer, funcs[T_CLEAR_SCREEN]);
    bytebuffer_puts(&output_buffer, funcs[T_EXIT_CA]);
    bytebuffer_puts(&output_buffer, funcs[T_EXIT_KEYPAD]);
    bytebuffer_puts(&output_buffer, funcs[T_EXIT_MOUSE]);
    bytebuffer_puts(&output_buffer, funcs[T_DISABLE_FOCUS_EVENTS]);
    bytebuffer_flush(&output_buffer, inout);
    tcsetattr(inout, TCSAFLUSH, &orig_tios);

    shutdown_term();
    close(inout);

    cellbuf_free(&back_buffer);
    cellbuf_free(&front_buffer);
    bytebuffer_free(&output_buffer);
    termw = termh = -1;
    cursor_x = -1;
    cursor_y = -1;
}

void tb_present(void)
{
    int x,y,w,i;
    struct tb_cell *back, *front;

    /* invalidate cursor position */
    lastx = LAST_COORD_INIT;
    lasty = LAST_COORD_INIT;

    if (buffer_size_change_request) {
        update_size();
        buffer_size_change_request = 0;
    }

    for (y = 0; y < front_buffer.height; ++y) {
        for (x = 0; x < front_buffer.width; ) {
            back = &CELL(&back_buffer, x, y);
            front = &CELL(&front_buffer, x, y);
            w = back->cw;
            if (w < 1) w = 1;
            if (memcmp(back, front, sizeof(struct tb_cell)) == 0) {
                x += w;
                continue;
            }
            memcpy(front, back, sizeof(struct tb_cell));
            send_attr(back->fg, back->bg);
            if (w > 1 && x >= front_buffer.width - (w - 1)) {
                // Not enough room for wide ch, so send spaces
                for (i = x; i < front_buffer.width; ++i) {
                    send_char(i, y, ' ');
                }
            } else {
                send_char(x, y, back->ch);
                for (i = 1; i < w; ++i) {
                    front = &CELL(&front_buffer, x + i, y);
                    front->ch = 0;
                    front->fg = back->fg;
                    front->bg = back->bg;
                }
            }
            x += w;
        }
    }
    if (!IS_CURSOR_HIDDEN(cursor_x, cursor_y))
        write_cursor(cursor_x, cursor_y);
    bytebuffer_flush(&output_buffer, inout);
}

void tb_set_cursor(int cx, int cy)
{
    if (IS_CURSOR_HIDDEN(cursor_x, cursor_y) && !IS_CURSOR_HIDDEN(cx, cy))
        bytebuffer_puts(&output_buffer, funcs[T_SHOW_CURSOR]);

    if (!IS_CURSOR_HIDDEN(cursor_x, cursor_y) && IS_CURSOR_HIDDEN(cx, cy))
        bytebuffer_puts(&output_buffer, funcs[T_HIDE_CURSOR]);

    cursor_x = cx;
    cursor_y = cy;
    if (!IS_CURSOR_HIDDEN(cursor_x, cursor_y))
        write_cursor(cursor_x, cursor_y);
}

void tb_put_cell(int x, int y, const struct tb_cell *cell)
{
    if ((unsigned)x >= (unsigned)back_buffer.width)
        return;
    if ((unsigned)y >= (unsigned)back_buffer.height)
        return;
    CELL(&back_buffer, x, y) = *cell;
}

void tb_change_cell(int x, int y, uint32_t ch, uint8_t cw, uint16_t fg, uint16_t bg)
{
    struct tb_cell c = {ch, fg, bg, cw};
    tb_put_cell(x, y, &c);
}

int tb_width(void)
{
    return termw;
}

int tb_height(void)
{
    return termh;
}

void tb_resize(void)
{
    buffer_size_change_request = 1;
}

void tb_clear(void)
{
    if (buffer_size_change_request) {
        update_size();
        buffer_size_change_request = 0;
    }
    cellbuf_clear(&back_buffer);
}

int tb_select_output_mode(int mode)
{
    if (mode)
        outputmode = mode;
    return outputmode;
}

void tb_set_clear_attributes(uint16_t fg, uint16_t bg)
{
    foreground = fg;
    background = bg;
}

/* -------------------------------------------------------- */

static int convertnum(uint32_t num, char* buf) {
    int i, l = 0;
    int ch;
    do {
        buf[l++] = '0' + (num % 10);
        num /= 10;
    } while (num);
    for(i = 0; i < l / 2; i++) {
        ch = buf[i];
        buf[i] = buf[l - 1 - i];
        buf[l - 1 - i] = ch;
    }
    return l;
}

#define WRITE_LITERAL(X) bytebuffer_append(&output_buffer, (X), sizeof(X)-1)
#define WRITE_INT(X) bytebuffer_append(&output_buffer, buf, convertnum((X), buf))

static void write_cursor(int x, int y) {
    char buf[32];
    WRITE_LITERAL("\033[");
    WRITE_INT(y+1);
    WRITE_LITERAL(";");
    WRITE_INT(x+1);
    WRITE_LITERAL("H");
}

static void write_sgr(uint16_t fg, uint16_t bg) {
    char buf[32];

    if (fg == TB_DEFAULT && bg == TB_DEFAULT)
        return;

    switch (outputmode) {
    case TB_OUTPUT_256:
    case TB_OUTPUT_216:
    case TB_OUTPUT_GRAYSCALE:
        WRITE_LITERAL("\033[");
        if (fg != TB_DEFAULT) {
            WRITE_LITERAL("38;5;");
            WRITE_INT(fg);
            if (bg != TB_DEFAULT) {
                WRITE_LITERAL(";");
            }
        }
        if (bg != TB_DEFAULT) {
            WRITE_LITERAL("48;5;");
            WRITE_INT(bg);
        }
        WRITE_LITERAL("m");
        break;
    case TB_OUTPUT_NORMAL:
    default:
        WRITE_LITERAL("\033[");
        if (fg != TB_DEFAULT) {
            WRITE_LITERAL("3");
            WRITE_INT(fg - 1);
            if (bg != TB_DEFAULT) {
                WRITE_LITERAL(";");
            }
        }
        if (bg != TB_DEFAULT) {
            WRITE_LITERAL("4");
            WRITE_INT(bg - 1);
        }
        WRITE_LITERAL("m");
        break;
    }
}

static void cellbuf_init(struct cellbuf *buf, int width, int height)
{
    buf->cells = (struct tb_cell*)malloc(sizeof(struct tb_cell) * width * height);
    assert(buf->cells);
    buf->width = width;
    buf->height = height;
}

static void cellbuf_resize(struct cellbuf *buf, int width, int height)
{
    if (buf->width == width && buf->height == height)
        return;

    int oldw = buf->width;
    int oldh = buf->height;
    struct tb_cell *oldcells = buf->cells;

    cellbuf_init(buf, width, height);
    cellbuf_clear(buf);

    int minw = (width < oldw) ? width : oldw;
    int minh = (height < oldh) ? height : oldh;
    int i;

    for (i = 0; i < minh; ++i) {
        struct tb_cell *csrc = oldcells + (i * oldw);
        struct tb_cell *cdst = buf->cells + (i * width);
        memcpy(cdst, csrc, sizeof(struct tb_cell) * minw);
    }

    free(oldcells);
}

static void cellbuf_clear(struct cellbuf *buf)
{
    int i;
    int ncells = buf->width * buf->height;

    for (i = 0; i < ncells; ++i) {
        buf->cells[i].ch = ' ';
        buf->cells[i].fg = foreground;
        buf->cells[i].bg = background;
    }
}

static void cellbuf_free(struct cellbuf *buf)
{
    free(buf->cells);
}

static void update_term_size(void)
{
    struct winsize sz;
    memset(&sz, 0, sizeof(sz));

    ioctl(inout, TIOCGWINSZ, &sz);

    termw = sz.ws_col;
    termh = sz.ws_row;
}

static void send_attr(uint16_t fg, uint16_t bg)
{
#define LAST_ATTR_INIT 0xFFFF
    static uint16_t lastfg = LAST_ATTR_INIT, lastbg = LAST_ATTR_INIT;
    if (fg != lastfg || bg != lastbg) {
        bytebuffer_puts(&output_buffer, funcs[T_SGR0]);

        uint16_t fgcol;
        uint16_t bgcol;

        switch (outputmode) {
        case TB_OUTPUT_256:
            fgcol = fg & 0xFF;
            bgcol = bg & 0xFF;
            break;

        case TB_OUTPUT_216:
            fgcol = fg & 0xFF; if (fgcol > 215) fgcol = 7;
            bgcol = bg & 0xFF; if (bgcol > 215) bgcol = 0;
            fgcol += 0x10;
            bgcol += 0x10;
            break;

        case TB_OUTPUT_GRAYSCALE:
            fgcol = fg & 0xFF; if (fgcol > 23) fgcol = 23;
            bgcol = bg & 0xFF; if (bgcol > 23) bgcol = 0;
            fgcol += 0xe8;
            bgcol += 0xe8;
            break;

        case TB_OUTPUT_NORMAL:
        default:
            fgcol = fg & 0x0F;
            bgcol = bg & 0x0F;
        }

        if (fg & TB_BOLD)
            bytebuffer_puts(&output_buffer, funcs[T_BOLD]);
        if (bg & TB_BOLD)
            bytebuffer_puts(&output_buffer, funcs[T_BLINK]);
        if (fg & TB_UNDERLINE)
            bytebuffer_puts(&output_buffer, funcs[T_UNDERLINE]);
        if ((fg & TB_REVERSE) || (bg & TB_REVERSE))
            bytebuffer_puts(&output_buffer, funcs[T_REVERSE]);

        write_sgr(fgcol, bgcol);

        lastfg = fg;
        lastbg = bg;
    }
}

static void send_char(int x, int y, uint32_t c)
{
    if (x-1 != lastx || y != lasty)
        write_cursor(x, y);
    lastx = x; lasty = y;
    bytebuffer_append_utf8_char(&output_buffer, c);
}

static void send_clear(void)
{
    send_attr(foreground, background);
    bytebuffer_puts(&output_buffer, funcs[T_CLEAR_SCREEN]);
    if (!IS_CURSOR_HIDDEN(cursor_x, cursor_y))
        write_cursor(cursor_x, cursor_y);
    bytebuffer_flush(&output_buffer, inout);

    /* we need to invalidate cursor position too and these two vars are
     * used only for simple cursor positioning optimization, cursor
     * actually may be in the correct place, but we simply discard
     * optimization once and it gives us simple solution for the case when
     * cursor moved */
    lastx = LAST_COORD_INIT;
    lasty = LAST_COORD_INIT;
}

static void update_size(void)
{
    update_term_size();
    cellbuf_resize(&back_buffer, termw, termh);
    cellbuf_resize(&front_buffer, termw, termh);
    cellbuf_clear(&front_buffer);
    send_clear();
}
