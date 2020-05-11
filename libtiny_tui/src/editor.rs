//! Implements C-x ("edit message in $EDITOR") support

use std::io::{Read, Seek, SeekFrom, Write};
use std::process::Command;
use termbox_simple::Termbox;

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

pub(crate) fn paste_lines(
    tb: &mut Termbox,
    mut text_field_contents: String,
    text_field_cursor: i32,
    pasted_string: &str,
) -> Result<Vec<String>, EditorError> {
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
    edit(tb, &contents)
}

pub(crate) fn edit(tb: &mut Termbox, contents: &str) -> Result<Vec<String>, EditorError> {
    let editor = ::std::env::var("EDITOR")?;
    let mut tmp_file = ::tempfile::NamedTempFile::new()?;

    write!(tmp_file, "{}", contents)?;

    tb.suspend();
    let ret = Command::new(editor).arg(tmp_file.path()).status();
    tb.activate();

    let ret = ret?;
    if !ret.success() {
        return Ok(vec![]); // assume aborted
    }

    let mut tmp_file = tmp_file.into_file();
    tmp_file.seek(SeekFrom::Start(0))?;

    let mut file_contents = String::new();
    tmp_file.read_to_string(&mut file_contents)?;

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

    Ok(filtered_lines)
}
