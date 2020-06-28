/// Cache that stores the state of a line's height calculation.
/// `line_count` is used as the dirty bit to invalidate the cache.
#[derive(Clone, Debug)]
pub struct LineDataCache {
    /// Indices to split on when we draw, not always whitespaces
    split_indices: Vec<i32>,
    /// The total number of lines (height) that will be rendered
    line_count: Option<i32>,
    /// The current width of InputArea. Used in determining if we need to invalidate due to resize.
    width: i32,
    /// The current width of the InputLine (may be shorter due to nickname)
    line_width: i32,
    /// Current nickname length. Used in determining if we need to invalidate due to resize.
    nick_length: usize,
    /// The index into InputLine::buffer of the last whitespace that we saw in calculate_height()
    last_whitespace_idx: Option<i32>,
    /// True if the last character was a whitespace character
    prev_char_is_whitespace: bool,
    /// The length of the current line that is being added to.
    /// Used to determine when to wrap to the next line in calculate_height()
    current_line_length: i32,
    /// If the line of text has a cursor
    has_cursor: bool,
}

impl LineDataCache {
    pub fn new(has_cursor: bool) -> LineDataCache {
        LineDataCache {
            split_indices: Vec::new(),
            line_count: None,
            width: 0,
            line_width: 0,
            nick_length: 0,
            last_whitespace_idx: None,
            prev_char_is_whitespace: false,
            current_line_length: 0,
            has_cursor,
        }
    }

    /// Performs a check to see if the width or nickname length changed
    /// which would require an invalidation of the cache and recalculation of
    /// the line height.
    pub fn needs_resize(&self, width: i32, nick_length: usize) -> bool {
        self.width != width || self.nick_length != nick_length
    }

    /// Sets `line_count` to `None`, which invalidates the cache.
    pub fn set_dirty(&mut self) {
        self.line_count = None;
    }

    /// Checks if the cache is invalidated by seeing if
    /// `line_count` is `None`.
    pub fn is_dirty(&self) -> bool {
        self.line_count.is_none()
    }

    /// Resets the cache to a default state that requires
    /// a height calculation.
    pub fn reset(&mut self, width: i32, nick_length: usize) {
        self.split_indices.clear();
        self.line_count = None;
        self.width = width;
        self.nick_length = nick_length;
        self.line_width = width - nick_length as i32;
        self.last_whitespace_idx = None;
        self.prev_char_is_whitespace = false;
        self.current_line_length = 0;
    }

    pub fn get_line_count(&self) -> Option<usize> {
        self.line_count.map(|c| c as usize)
    }

    pub fn get_splits(&self) -> &[i32] {
        &self.split_indices
    }

    /// Function that calculates the height of the line.
    /// and sets `split_indices` for drawing.
    /// An `offset` allows for resuming the calculation - see InputLine::insert().
    /// `offset` must be less than or equal to the current buffer size.
    ///
    /// Scans through the buffer in one pass to determine how many lines
    /// will be needed to render the text with word wrapping.
    /// If an offset is provided, it will continue the calculation
    /// from the saved state and save the new line count in `line_count`.
    pub fn calculate_height<I: Iterator<Item = char>>(&mut self, buffer: I, offset: usize) {
        let mut temp_count = 1;
        if let Some(line_count) = self.line_count {
            temp_count = line_count;
            // If we made space for the cursor, subtract it.
            if self.has_cursor && self.current_line_length == self.line_width {
                temp_count -= 1;
            }
        }
        for (c, current_idx) in buffer.skip(offset).zip(offset..) {
            let current_idx = current_idx as i32;
            self.current_line_length += 1;

            if c.is_whitespace() {
                // Splitting
                if self.current_line_length > self.line_width {
                    // we're on a whitespace so just go to next line
                    temp_count += 1;
                    // this character will be the first one on the next line
                    self.current_line_length = 1;
                    // nick is shown on the first line, set width to full width in the consecutive
                    // lines
                    self.line_width = self.width;
                    // store index for drawing
                    self.split_indices.push(current_idx);
                }
                // store whitespace for splitting
                self.last_whitespace_idx = Some(current_idx);
                self.prev_char_is_whitespace = true;
            } else {
                // Splitting
                if self.current_line_length > self.line_width {
                    // if the previous character was a whitespace, then we have a clean split
                    if !self.prev_char_is_whitespace && self.last_whitespace_idx.is_some() {
                        // move back to the last whitespace and get the length of the input that
                        // will be on the next line
                        self.current_line_length = current_idx - self.last_whitespace_idx.unwrap();
                        // store index for drawing
                        self.split_indices
                            .push(self.last_whitespace_idx.unwrap() + 1);
                    } else {
                        // unclean split on non-whitespace
                        self.current_line_length = 1;
                        // store index for drawing
                        self.split_indices.push(current_idx);
                    }
                    // invalidate whitespace since we split here
                    self.last_whitespace_idx = None;
                    // moved to next line
                    temp_count += 1;
                    // set width to full width
                    self.line_width = self.width;
                }
                self.prev_char_is_whitespace = false;
            }
        }

        // Last line length is `line_width`, make room for cursor
        if self.has_cursor && self.current_line_length == self.line_width {
            temp_count += 1;
        }
        self.line_count = Some(temp_count);
    }
}
