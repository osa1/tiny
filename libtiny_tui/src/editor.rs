//! Implements C-x ("edit message in $EDITOR") support

use std::io::{Read, Seek, SeekFrom, Write};
use std::process::{Command, ExitStatus};
use termbox_simple::Termbox;
use tokio::sync::oneshot;
use tokio::task::{spawn_blocking, JoinHandle};

#[derive(Debug)]
pub(crate) enum EditorError {
    Io(::std::io::Error),
    Var(::std::env::VarError),
    Recv(oneshot::error::RecvError),
}

impl From<::std::io::Error> for EditorError {
    fn from(err: ::std::io::Error) -> EditorError {
        EditorError::Io(err)
    }
}

impl From<::std::env::VarError> for EditorError {
    fn from(err: ::std::env::VarError) -> EditorError {
        EditorError::Var(err)
    }
}

impl From<oneshot::error::RecvError> for EditorError {
    fn from(err: oneshot::error::RecvError) -> EditorError {
        EditorError::Recv(err)
    }
}

type Result<A> = std::result::Result<A, EditorError>;

pub(crate) async fn paste_lines(
    tb: &mut Termbox,
    mut text_field_contents: String,
    text_field_cursor: i32,
    pasted_string: &str,
) -> Result<Vec<String>> {
    let mut contents =
        "# You pasted a multi-line message. When you close the editor final version of\n\
         # this file will be sent (ignoring these lines). Delete contents to abort the\n\
         # paste.\n"
            .to_string();
    text_field_contents.insert_str(
        text_field_cursor as usize,
        &pasted_string.replace('\r', "\n"),
    );
    contents.push_str(&text_field_contents);
    edit(tb, &contents).await
}

pub(crate) async fn edit(tb: &mut Termbox, contents: &str) -> Result<Vec<String>> {
    let editor = ::std::env::var("EDITOR")?;
    let mut tmp_file = ::tempfile::NamedTempFile::new()?;

    write!(tmp_file, "{}", contents)?;

    let (snd_result, rcv_result): (
        oneshot::Sender<Result<Vec<String>>>,
        oneshot::Receiver<Result<Vec<String>>>,
    ) = oneshot::channel();

    // Idea: suspend termbox, spawn a thread (not a tokio task!) for running $EDITOR so that tokio
    // event loop won't be blocked. Current task (input_handler) will be blocked to hear from the
    // thread, but that's fine as we won't be getting input events while user is inside $EDITOR.

    tb.suspend();
    let thread: JoinHandle<()> = spawn_blocking(|| {
        let ret: std::result::Result<ExitStatus, std::io::Error> =
            Command::new(editor).arg(tmp_file.path()).status();

        let ret = match ret {
            Ok(ret) => ret,
            Err(err) => {
                snd_result.send(Err(err.into()));
                return;
            }
        };

        if !ret.success() {
            snd_result.send(Ok(vec![])); // assume aborted
            return;
        }

        let mut tmp_file = tmp_file.into_file();
        let io_ret = tmp_file.seek(SeekFrom::Start(0));
        if let Err(err) = io_ret {
            snd_result.send(Err(err.into()));
            return;
        }

        let mut file_contents = String::new();
        let io_ret = tmp_file.read_to_string(&mut file_contents);
        if let Err(err) = io_ret {
            snd_result.send(Err(err.into()));
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

        snd_result.send(Ok(filtered_lines));
    });

    let ret = rcv_result.await;
    tb.activate();
    ret?
}
