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

#[derive(Debug)]
pub(crate) enum Error {
    Io(::std::io::Error),
    Var(::std::env::VarError),
}

impl From<::std::io::Error> for Error {
    fn from(err: ::std::io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<::std::env::VarError> for Error {
    fn from(err: ::std::env::VarError) -> Error {
        Error::Var(err)
    }
}

pub(crate) type Result<A> = ::std::result::Result<A, Error>;

pub(crate) type ResultReceiver = oneshot::Receiver<Result<Vec<String>>>;

pub(crate) fn run(
    tb: &mut Termbox,
    text_field_contents: &str,
    pasted_text: &str,
    rcv_editor_ret: &mut Option<ResultReceiver>,
) -> Result<()> {
    use std::{
        io::{Read, Seek, SeekFrom, Write},
        process::Command,
    };

    let editor = ::std::env::var("EDITOR")?;
    let mut tmp_file = ::tempfile::NamedTempFile::new()?;

    writeln!(
        tmp_file,
        "\
         # You pasted a multi-line message. When you close the editor final version of\n\
         # this file will be sent (ignoring these lines). Delete contents to abort the\n\
         # paste."
    )?;
    write!(tmp_file, "{}", text_field_contents)?;
    write!(tmp_file, "{}", pasted_text.replace('\r', "\n"))?;

    let (snd_editor_ret, rcv_editor_ret_) = oneshot::channel();
    *rcv_editor_ret = Some(rcv_editor_ret_);

    // Will be activated by the input handler task after reading rcv_editor_ret
    tb.suspend();

    // At this point termbox is suspended (terminal settings restored), and the input handler task
    // won't be reading term_input until it reads from rcv_editor_ret, so the editor has control
    // over the tty, stdout, and stdin.
    tokio::task::spawn_blocking(move || {
        let ret = Command::new(editor).arg(tmp_file.path()).status();
        let ret = match ret {
            Err(io_err) => {
                snd_editor_ret.send(Err(io_err.into())).unwrap();
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
            snd_editor_ret.send(Err(io_err.into())).unwrap();
            return;
        }

        let mut file_contents = String::new();
        if let Err(io_err) = tmp_file.read_to_string(&mut file_contents) {
            snd_editor_ret.send(Err(io_err.into())).unwrap();
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
