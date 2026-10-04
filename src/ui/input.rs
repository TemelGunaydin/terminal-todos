use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug)]
pub struct Editor {
    pub text: String,
    cursor: usize,
}

impl Editor {
    pub fn new(text: String) -> Self {
        Self {
            cursor: text.len(),
            text,
        }
    }

    pub fn insert(&mut self, text: &str) {
        let text = crate::model::safe(text);
        self.text.insert_str(self.cursor, &text);
        self.cursor += text.len();
        // A combining character or ZWJ can merge with the next grapheme.
        self.cursor = self
            .text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|&i| i >= self.cursor)
            .unwrap_or(self.text.len());
    }

    pub fn key(&mut self, key: KeyEvent) {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('u') if control => {
                self.text.drain(..self.cursor);
                self.cursor = 0;
            }
            KeyCode::Char('a') if control => self.cursor = 0,
            KeyCode::Char('e') if control => self.cursor = self.text.len(),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.text.len(),
            KeyCode::Left => self.cursor = self.previous(),
            KeyCode::Right => self.cursor = self.next(),
            KeyCode::Backspace => {
                let previous = self.previous();
                self.text.drain(previous..self.cursor);
                self.cursor = previous;
            }
            KeyCode::Delete => {
                self.text.drain(self.cursor..self.next());
            }
            KeyCode::Char(ch)
                if !control && !key.modifiers.contains(KeyModifiers::ALT) && !ch.is_control() =>
            {
                self.insert(&ch.to_string())
            }
            _ => {}
        }
    }

    fn previous(&self) -> usize {
        self.text[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn next(&self) -> usize {
        self.text[self.cursor..]
            .graphemes(true)
            .next()
            .map_or(self.cursor, |g| self.cursor + g.len())
    }

    /// Return a grapheme-safe, horizontally scrolled line and its cursor column.
    pub fn view(&self, width: u16) -> (String, u16) {
        if width == 0 {
            return (String::new(), 0);
        }
        let before = self.text[..self.cursor].width();
        let target = before.saturating_sub(width.saturating_sub(1) as usize);
        let mut start = 0;
        let mut skipped = 0;
        for (i, grapheme) in self.text.grapheme_indices(true) {
            if skipped >= target {
                start = i;
                break;
            }
            skipped += grapheme.width();
            start = i + grapheme.len();
        }
        let mut visible = String::new();
        let mut used = 0;
        for grapheme in self.text[start..].graphemes(true) {
            let columns = grapheme.width();
            if used + columns > width as usize {
                break;
            }
            visible.push_str(grapheme);
            used += columns;
        }
        (
            visible,
            before
                .saturating_sub(skipped)
                .min(width.saturating_sub(1) as usize) as u16,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn unicode_cursor_and_deletion_use_whole_graphemes() {
        let mut input = Editor::new("şe\u{301}👨‍👩‍👧‍👦".into());
        input.key(key(KeyCode::Backspace));
        assert_eq!(input.text, "şe\u{301}");
        input.key(key(KeyCode::Left));
        input.insert("🦀");
        assert_eq!(input.text, "ş🦀e\u{301}");
        input.key(key(KeyCode::Delete));
        assert_eq!(input.text, "ş🦀");
    }

    #[test]
    fn long_input_keeps_cursor_inside_the_viewport() {
        let input = Editor::new("Türkçe görev 🦀 abcdefghijklmnop".into());
        let (view, cursor) = input.view(12);
        assert!(view.width() <= 12);
        assert!(cursor < 12);
        assert!(view.ends_with("mnop"));
    }

    #[test]
    fn paste_cannot_inject_terminal_controls() {
        let mut input = Editor::new(String::new());
        input.insert("one\ntwo\u{1b}[2J");
        assert!(!input.text.chars().any(char::is_control));
    }
}
