/// Cache that stores the state of a line's height calculation.
/// `line_count` is used as the dirty bit to invalidate the cache.
#[derive(Clone, Debug)]
pub(crate) struct LineDataCache {
    /// Indices to split on when we draw, not always whitespaces
    split_indices: Vec<i32>,
    /// The total number of lines (height) that will be rendered
    line_count: Option<i32>,
    /// The current width of InputArea. Used in determining if we need to invalidate due to resize.
    width: i32,
    /// The width of the current line (full width minus nick_length or msg padding)
    line_width: i32,
    /// The index into InputLine::buffer of the last whitespace that we saw in calculate_height()
    last_whitespace_idx: Option<i32>,
    /// The length of the current line that is being added to.
    /// Used to determine when to wrap to the next line in calculate_height()
    current_line_length: i32,
    line_type: LineType,
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum LineType {
    Input {
        /// Current nickname length. Used in determining if we need to invalidate due to resize.
        nick_length: usize,
    },
    AlignedMsg {
        /// The offset of timestamp + nick on multiline messages
        msg_padding: usize,
    },
    Msg,
}

impl LineType {
    pub(crate) fn msg_padding(&self) -> Option<usize> {
        if let LineType::AlignedMsg { msg_padding } = self {
            Some(*msg_padding)
        } else {
            None
        }
    }
}

impl LineDataCache {
    pub(crate) fn input_line(width: i32, nick_length: usize) -> LineDataCache {
        LineDataCache {
            split_indices: Vec::new(),
            line_count: None,
            width,
            line_width: width - nick_length as i32,
            last_whitespace_idx: None,
            current_line_length: 0,
            line_type: LineType::Input { nick_length },
        }
    }

    pub(crate) fn msg_line(width: i32, msg_padding: Option<usize>) -> LineDataCache {
        let line_type = if let Some(msg_padding) = msg_padding {
            LineType::AlignedMsg { msg_padding }
        } else {
            LineType::Msg
        };
        LineDataCache {
            split_indices: Vec::new(),
            line_count: None,
            width,
            line_width: width,
            last_whitespace_idx: None,
            current_line_length: 0,
            line_type,
        }
    }

    pub(crate) fn line_type(&self) -> LineType {
        self.line_type
    }

    pub(crate) fn set_line_type(&mut self, line_type: LineType) {
        self.line_type = line_type
    }

    /// Performs a check to see if the width or nickname length changed
    /// which would require an invalidation of the cache and recalculation of
    /// the line height.
    pub(crate) fn needs_resize(&self, width: i32, nick_len: usize, msg_pad: Option<usize>) -> bool {
        // TODO so ugly
        self.width != width
            || match (self.line_type, msg_pad) {
                (LineType::Input { nick_length }, ..) => nick_len != nick_length,
                (LineType::AlignedMsg { msg_padding }, Some(pad)) => pad != msg_padding,
                _ => false,
            }
    }

    /// Sets `line_count` to `None`, which invalidates the cache.
    pub(crate) fn set_dirty(&mut self) {
        self.line_count = None;
    }

    /// Checks if the cache is invalidated by seeing if
    /// `line_count` is `None`.
    pub(crate) fn is_dirty(&self) -> bool {
        self.line_count.is_none()
    }

    pub(crate) fn get_line_count(&self) -> Option<usize> {
        self.line_count.map(|c| c as usize)
    }

    pub(crate) fn get_splits(&self) -> &[i32] {
        &self.split_indices
    }

    pub(crate) fn new_line_offset(&self) -> i32 {
        match self.line_type {
            LineType::Input { .. } | LineType::Msg => 0,
            LineType::AlignedMsg { msg_padding } => msg_padding as i32,
        }
    }

    fn multi_line_width(&self) -> i32 {
        match self.line_type {
            // expand to full width
            LineType::Input { .. } | LineType::Msg => self.width,
            // consecutive lines can apply an offset
            LineType::AlignedMsg { msg_padding } => self.width - msg_padding as i32,
        }
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
    pub(crate) fn calculate_height<I: Iterator<Item = char>>(&mut self, buffer: I, offset: usize) {
        let mut temp_count = 1;
        if let Some(line_count) = self.line_count {
            temp_count = line_count;
            // If we made space for the cursor, subtract it.
            if let LineType::Input { .. } = self.line_type {
                if self.current_line_length == self.line_width {
                    temp_count -= 1;
                }
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
                    self.line_width = self.multi_line_width();

                    // store index for drawing
                    self.split_indices.push(current_idx);
                }
                // store whitespace for splitting
                self.last_whitespace_idx = Some(current_idx);
            } else {
                // Splitting on non-whitespace
                if self.current_line_length > self.line_width {
                    // set width to full width
                    self.line_width = self.multi_line_width();
                    // if the previous character was a whitespace, then we have a clean split
                    if let Some(last_whitespace_idx) = self.last_whitespace_idx {
                        // if the split is larger than the width we have,
                        // we just want to do an unclean split (mainly only for links or if someone spams a super long line)
                        if current_idx - last_whitespace_idx > self.line_width {
                            // unclean split on non-whitespace
                            self.current_line_length = 1;
                            // store index for drawing
                            self.split_indices.push(current_idx);
                        } else {
                            // move back to the last whitespace and get the length of the input that
                            // will be on the next line
                            self.current_line_length = current_idx - last_whitespace_idx;

                            // store index for drawing
                            self.split_indices.push(last_whitespace_idx + 1);
                        }
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
                }
            }
        }

        // Last line length is `line_width`, make room for cursor
        if let LineType::Input { .. } = self.line_type {
            if self.current_line_length == self.line_width {
                temp_count += 1;
            }
        }
        self.line_count = Some(temp_count);
    }
}
