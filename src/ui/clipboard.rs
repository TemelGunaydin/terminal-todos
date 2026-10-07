use crossterm::{clipboard::CopyToClipboard, execute};
use std::io::{self, Write};

/// Request the terminal host's clipboard, including remote/SSH sessions.
/// OSC52 has no acknowledgement: a successful write does not prove the host accepted it.
pub fn copy(writer: &mut impl Write, text: &str) -> io::Result<()> {
    execute!(writer, CopyToClipboard::to_clipboard_from(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_full_utf8_text_not_a_display_snippet() -> io::Result<()> {
        let text = "Türkçe görev 🦀 ".repeat(200);
        let mut actual = Vec::new();
        copy(&mut actual, &text)?;
        use crossterm::Command;
        let mut expected = String::new();
        CopyToClipboard::to_clipboard_from(&text)
            .write_ansi(&mut expected)
            .unwrap();
        assert_eq!(actual, expected.as_bytes());
        assert!(actual.starts_with(b"\x1b]52;c;"));
        assert!(actual.ends_with(b"\x1b\\"));
        assert!(actual.len() > text.len());
        Ok(())
    }

    #[test]
    fn clipboard_write_failures_are_reported() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "fixture"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert!(copy(&mut Broken, "Keep me").is_err());
    }
}
