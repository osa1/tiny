#pragma once

#include <stdint.h>

/* for shared objects */
#if __GNUC__ >= 4
 #define SO_IMPORT __attribute__((visibility("default")))
#else
 #define SO_IMPORT
#endif

#ifdef __cplusplus
extern "C" {
#endif

/* Colors (see struct tb_cell's fg and bg fields). */
#define TB_DEFAULT 0x00
#define TB_BLACK   0x01
#define TB_RED     0x02
#define TB_GREEN   0x03
#define TB_YELLOW  0x04
#define TB_BLUE    0x05
#define TB_MAGENTA 0x06
#define TB_CYAN    0x07
#define TB_WHITE   0x08

/* Attributes, it is possible to use multiple attributes by combining them
 * using bitwise OR ('|'). Although, colors cannot be combined. But you can
 * combine attributes and a single color. See also struct tb_cell's fg and bg
 * fields.
 */
#define TB_BOLD      0x0100
#define TB_UNDERLINE 0x0200
#define TB_REVERSE   0x0400

/* A cell, single conceptual entity on the terminal screen. The terminal screen
 * is basically a 2d array of cells. It has the following fields:
 *  - 'ch' is a unicode character
 *  - 'fg' foreground color and attributes
 *  - 'bg' background color and attributes
 *  - 'cw' is the visible width of the char
 */
struct tb_cell {
    uint32_t ch;
    uint16_t fg;
    uint16_t bg;
    uint8_t cw;
};

/* Error codes returned by tb_init(). All of them are self-explanatory, except
 * the pipe trap error. Termbox uses unix pipes in order to deliver a message
 * from a signal handler (SIGWINCH) to the main event reading loop. Honestly in
 * most cases you should just check the returned code as < 0.
 */
#define TB_EUNSUPPORTED_TERMINAL -1
#define TB_EFAILED_TO_OPEN_TTY   -2

/* Initializes the termbox library. This function should be called before any
 * other functions. After successful initialization, the library must be
 * finalized using the tb_shutdown() function.
 */
SO_IMPORT int tb_init(void);
SO_IMPORT void tb_resize(void);
SO_IMPORT void tb_shutdown(void);

/* Returns the size of the internal back buffer (which is the same as
 * terminal's window size in characters). The internal buffer can be resized
 * after tb_clear() or tb_present() function calls. Both dimensions have an
 * unspecified negative value when called before tb_init() or after
 * tb_shutdown().
 */
SO_IMPORT int tb_width(void);
SO_IMPORT int tb_height(void);

/* Clears the internal back buffer using TB_DEFAULT color or the
 * color/attributes set by tb_set_clear_attributes() function.
 */
SO_IMPORT void tb_clear(void);
SO_IMPORT void tb_set_clear_attributes(uint16_t fg, uint16_t bg);

/* Synchronizes the internal back buffer with the terminal. */
SO_IMPORT void tb_present(void);

#define TB_HIDE_CURSOR -1

/* Sets the position of the cursor. Upper-left character is (0, 0). If you pass
 * TB_HIDE_CURSOR as both coordinates, then the cursor will be hidden. Cursor
 * is hidden by default.
 */
SO_IMPORT void tb_set_cursor(int cx, int cy);

/* Changes cell's parameters in the internal back buffer at the specified
 * position.
 */
SO_IMPORT void tb_change_cell(int x, int y, uint32_t ch, uint8_t cw, uint16_t fg, uint16_t bg);

#ifdef __cplusplus
}
#endif
