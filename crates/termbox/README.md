This is an implementation of [termbox][1]'s "cell" abstraction in Rust.

Coordinates refer to terminal columns. A wide character occupies two cells;
writing over either cell clears the whole character. If only one column is
available, it is displayed as `�`. Use `append_combining` to attach a
zero-width character and its style to a previously written base cell, and
`set_cell_attributes` to change the base style without discarding its marks.
An isolated zero-width character can be written with `change_cell`, which
displays a dotted circle before it.

Character widths follow `unicode-width`'s default `UnicodeWidthChar::width()`
rules, including one column for ambiguous-width characters. Terminals
configured to render ambiguous characters as two columns may not align.

[1]: https://github.com/nsf/termbox
