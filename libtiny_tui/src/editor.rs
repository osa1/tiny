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

/// The user tried to paste the multi-line string passed as the argument. Run $EDITOR to edit a
/// temporary file with the string as the contents. On exit, parse the final contents of the file
/// (ignore comment lines), and send each line in the file as a message. Abort if any of the lines
/// look like a command (e.g. `/msg ...`). I don't know what's the best way to handle commands in
/// this context.
///
/// Ok(str) => final string to send
/// Err(str) => err message to show
///
/// FIXME: Ideally this function should get a `Termbox` argument and return a new `Termbox` because
/// we shutdown the current termbox instance and initialize it again after running $EDITOR.
pub(crate) fn edit(tb: &mut Termbox, tf: String, str: &str) -> Result<Vec<String>, EditorError> {
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
    write!(tmp_file, "{}", tf)?;
    write!(tmp_file, "{}", str.replace('\r', "\n"))?;

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
