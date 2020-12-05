//! This module implements some part of running $EDITOR to edit input. The other part is
//! implemented at the top-level of the current crate (in `input_handler` task).
//!
//! Implementation is a little bit hacky: we need to give control of tty, stdout, and stdin to
//! $EDITOR while still running the tokio event loop, to handle connection events. Here's how we
//! implement this:
//!
//! - $EDITOR runs in a new thread to avoid blocking tokio event loop.
//!
//! - termbox implement a 'suspend' method that restores the terminal and stops using tty, until
//!   'activate' is called. We can still use termbox methods as usual, but termbox doesn't really
//!   draw anything. This way we don't need to stop rendering TUI when $EDITOR is running, we just
//!   'suspend' termbox and keep running the TUI as usual in its own task.
//!
//! - We use a oneshot channel from the spawned $EDITOR thread to the TUI input handler task, to
//!   send the result. When the channel is available (not `None`) it means $EDITOR is running and
//!   we should not read stdin. So the stdin handler task just blocks on this channel when it is
//!   available. This happens in `input_handler` task at the top-level of the crate.

use termbox_simple::Termbox;
use tokio::sync::oneshot;

use std::io::{Read, Seek, SeekFrom, Write};
use std::process::Command;

#[derive(Debug)]
pub(crate) struct Error {
    /// Original contents of the text field passed to `editor::run`. This should be used to restore
    /// text field contents in case of an error as otherwise the input will be lost.
    pub(crate) text_field_contents: String,
    /// Original location of the cursor in the text field. This should be used to restore the
    /// cursor location on error.
    pub(crate) cursor: i32,
    /// The actual error
    pub(crate) kind: ErrorKind,
}

#[derive(Debug)]
pub(crate) enum ErrorKind {
    Io(::std::io::Error),
    Var(::std::env::VarError),
}

impl Error {
    fn new(text_field_contents: String, cursor: i32, kind: ErrorKind) -> Error {
        Error {
            text_field_contents,
            cursor,
            kind,
        }
    }
}

impl From<::std::io::Error> for ErrorKind {
    fn from(err: ::std::io::Error) -> ErrorKind {
        ErrorKind::Io(err)
    }
}

impl From<::std::env::VarError> for ErrorKind {
    fn from(err: ::std::env::VarError) -> ErrorKind {
        ErrorKind::Var(err)
    }
}

pub(crate) type Result<A> = ::std::result::Result<A, Error>;

pub(crate) type ResultReceiver = oneshot::Receiver<Result<Vec<String>>>;

/// `cursor` is the cursor location in `text_field_contents`.
pub(crate) fn run(
    tb: &mut Termbox,
    text_field_contents: String,
    cursor: i32, // location of cursor in `text_field_contents`
    pasted_text: &str,
    rcv_editor_ret: &mut Option<ResultReceiver>,
) -> Result<()> {
    let editor = match std::env::var("EDITOR") {
        Ok(editor) => editor,
        Err(err) => {
            return Err(Error::new(text_field_contents, cursor, err.into()));
        }
    };

    let mut tmp_file = match tempfile::NamedTempFile::new() {
        Ok(tmp_file) => tmp_file,
        Err(err) => {
            return Err(Error::new(text_field_contents, cursor, err.into()));
        }
    };

    // We'll be inserting the pasted text at `cursor`, find the split point (byte offset)
    let split_byte_idx = match text_field_contents.char_indices().nth(cursor as usize) {
        Some((byte_idx, _)) => byte_idx,
        None => text_field_contents.len(),
    };

    let (before_paste, after_paste) = text_field_contents.split_at(split_byte_idx);

    let write_ret = write!(
        tmp_file,
        "# You pasted a multi-line message. When you close the editor final version of\n\
         # this file will be sent (ignoring these lines). Delete contents to abort the\n\
         # paste.\n\
         {}{}{}",
        before_paste,
        pasted_text.replace('\r', "\n"),
        after_paste,
    );

    if let Err(err) = write_ret {
        return Err(Error::new(text_field_contents, cursor, err.into()));
    }

    let (snd_editor_ret, rcv_editor_ret_) = oneshot::channel();
    *rcv_editor_ret = Some(rcv_editor_ret_);

    // Will be activated by the input handler task after reading rcv_editor_ret
    tb.suspend();

    // At this point termbox is suspended (terminal settings restored), and the input handler task
    // won't be reading term_input until it reads from rcv_editor_ret, so the editor has control
    // over the tty, stdout, and stdin.
    tokio::task::spawn_blocking(move || {
        let mut cmd = Command::new(&editor);

        // If the editor is (n)vim or emacs, pass parameters to move the cursor to its location in
        // `text_field_contents`
        match editor.as_ref() {
            "vim" | "nvim" => {
                if cursor == 0 {
                    cmd.arg("-c").arg("normal! 3j");
                } else {
                    cmd.arg("-c").arg(format!("normal! 3j{}l", cursor));
                }
            }
            "emacs" => {
                cmd.arg(format!("+4:{}", cursor + 1));
            }
            _ => {}
        }

        let ret = cmd.arg(tmp_file.path()).status();
        let ret = match ret {
            Err(io_err) => {
                snd_editor_ret
                    .send(Err(Error::new(text_field_contents, cursor, io_err.into())))
                    .unwrap();
                return;
            }
            Ok(ret) => ret,
        };

        if !ret.success() {
            // Assume aborted
            snd_editor_ret.send(Ok(vec![])).unwrap();
            return;
        }

        let mut tmp_file = tmp_file.into_file();
        if let Err(io_err) = tmp_file.seek(SeekFrom::Start(0)) {
            snd_editor_ret
                .send(Err(Error::new(text_field_contents, cursor, io_err.into())))
                .unwrap();
            return;
        }

        let mut file_contents = String::new();
        if let Err(io_err) = tmp_file.read_to_string(&mut file_contents) {
            snd_editor_ret
                .send(Err(Error::new(text_field_contents, cursor, io_err.into())))
                .unwrap();
            return;
        }

        let mut filtered_lines = vec![];
        for s in file_contents.lines() {
            // Ignore if the char is '#'. To actually send a `#` add space.
            // For empty lines, send " ".
            let first_char = s.chars().next();
            if first_char == Some('#') {
                // skip this line
                continue;
            } else if s.is_empty() {
                filtered_lines.push(" ".to_owned());
            } else {
                filtered_lines.push(s.to_owned());
            }
        }

        snd_editor_ret.send(Ok(filtered_lines)).unwrap();
    });

    Ok(())
}
