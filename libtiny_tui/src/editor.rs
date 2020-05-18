//! Implements C-x ("edit message in $EDITOR") support

use std::fmt;
use std::io::{Read, Seek, SeekFrom, Write};
use std::process::{Command, ExitStatus};
use std::thread::spawn;
use termbox_simple::Termbox;
use tokio::sync::mpsc;

#[derive(Debug)]
pub(crate) enum EditorError {
    Io(::std::io::Error),
    Var(::std::env::VarError),
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

pub(crate) type Result = std::result::Result<Vec<String>, EditorError>;

pub(crate) fn edit(tb: &mut Termbox, contents: &str, mut snd_editor_out: mpsc::Sender<Result>) {
    let editor = match ::std::env::var("EDITOR") {
        Err(err) => {
            snd_editor_out.try_send(Err(err.into())).unwrap();
            return;
        }
        Ok(editor) => editor,
    };

    let mut tmp_file = match ::tempfile::NamedTempFile::new() {
        Err(err) => {
            snd_editor_out.try_send(Err(err.into())).unwrap();
            return;
        }
        Ok(tmp_file) => tmp_file,
    };

    if let Err(err) = write!(tmp_file, "{}", contents) {
        snd_editor_out.try_send(Err(err.into())).unwrap();
        return;
    }

    // TODO: Document the idea

    tb.suspend(); // activated later in `handle_editor_out`
    spawn(move || {
        let ret: std::result::Result<ExitStatus, std::io::Error> =
            Command::new(editor).arg(tmp_file.path()).status();

        let ret = match ret {
            Ok(ret) => ret,
            Err(err) => {
                snd_editor_out.try_send(Err(err.into())).unwrap();
                return;
            }
        };

        if !ret.success() {
            snd_editor_out.try_send(Ok(vec![])).unwrap(); // assume aborted
            return;
        }

        let mut tmp_file = tmp_file.into_file();
        let io_ret = tmp_file.seek(SeekFrom::Start(0));
        if let Err(err) = io_ret {
            snd_editor_out.try_send(Err(err.into())).unwrap();
            return;
        }

        let mut file_contents = String::new();
        let io_ret = tmp_file.read_to_string(&mut file_contents);
        if let Err(err) = io_ret {
            snd_editor_out.try_send(Err(err.into())).unwrap();
            return;
        }

        snd_editor_out
            .try_send(Ok(file_contents.lines().map(str::to_owned).collect()))
            .unwrap();
    });
}
