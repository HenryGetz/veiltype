use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorAction {
    Continue,
    SaveAndExit,
    Cancel,
}

#[derive(Debug, Default, Clone)]
pub struct EditorState {
    buffer: String,
    cursor: usize,
    desired_col: Option<usize>,
}

impl EditorState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn line_col(&self) -> (usize, usize) {
        line_col_at(&self.buffer, self.cursor)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> EditorAction {
        match (key.code, key.modifiers) {
            (KeyCode::Char('s'), mods) if mods.contains(KeyModifiers::CONTROL) => {
                EditorAction::SaveAndExit
            }
            (KeyCode::Char('q'), mods) if mods.contains(KeyModifiers::CONTROL) => {
                EditorAction::Cancel
            }
            (KeyCode::Char('z'), mods) if mods.contains(KeyModifiers::CONTROL) => {
                EditorAction::Cancel
            }
            (KeyCode::Esc, _) => EditorAction::Cancel,
            (KeyCode::Backspace, mods) if mods.contains(KeyModifiers::ALT) => {
                self.delete_word_back();
                EditorAction::Continue
            }
            (KeyCode::Char('w'), mods) if mods.contains(KeyModifiers::CONTROL) => {
                self.delete_word_back();
                EditorAction::Continue
            }
            (KeyCode::Backspace, _) => {
                self.backspace();
                EditorAction::Continue
            }
            (KeyCode::Delete, _) => {
                self.delete();
                EditorAction::Continue
            }
            (KeyCode::Enter, _) => {
                self.insert_char('\n');
                EditorAction::Continue
            }
            (KeyCode::Tab, _) => {
                self.insert_str("    ");
                EditorAction::Continue
            }
            (KeyCode::Left, _) => {
                self.move_left();
                EditorAction::Continue
            }
            (KeyCode::Right, _) => {
                self.move_right();
                EditorAction::Continue
            }
            (KeyCode::Up, _) => {
                self.move_up();
                EditorAction::Continue
            }
            (KeyCode::Down, _) => {
                self.move_down();
                EditorAction::Continue
            }
            (KeyCode::Home, _) => {
                self.move_home();
                EditorAction::Continue
            }
            (KeyCode::End, _) => {
                self.move_end();
                EditorAction::Continue
            }
            (KeyCode::Char(c), mods)
                if !mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::SUPER) =>
            {
                self.insert_char(c);
                EditorAction::Continue
            }
            _ => EditorAction::Continue,
        }
    }

    pub fn move_left(&mut self) {
        if let Some(prev) = prev_char_start(&self.buffer, self.cursor) {
            self.cursor = prev;
            self.desired_col = None;
        }
    }

    pub fn move_right(&mut self) {
        if let Some(next) = next_char_end(&self.buffer, self.cursor) {
            self.cursor = next;
            self.desired_col = None;
        }
    }

    pub fn move_home(&mut self) {
        let (line_start, _) = line_bounds_at(&self.buffer, self.cursor);
        self.cursor = line_start;
        self.desired_col = None;
    }

    pub fn move_end(&mut self) {
        let (_, line_end) = line_bounds_at(&self.buffer, self.cursor);
        self.cursor = line_end;
        self.desired_col = None;
    }

    pub fn move_up(&mut self) {
        let (line, col) = self.line_col();
        if line == 0 {
            return;
        }

        let target_col = self.desired_col.get_or_insert(col);
        let prev_line_start = line_start(&self.buffer, line - 1);
        let prev_line_end = line_end(&self.buffer, prev_line_start);
        self.cursor = byte_at_visual_col(&self.buffer, prev_line_start, prev_line_end, *target_col);
    }

    pub fn move_down(&mut self) {
        let (line, col) = self.line_col();
        let current_line_start = line_start(&self.buffer, line);
        let current_line_end = line_end(&self.buffer, current_line_start);
        if current_line_end >= self.buffer.len() {
            return;
        }

        let target_col = self.desired_col.get_or_insert(col);
        let next_line_start = current_line_end + 1;
        let next_line_end = line_end(&self.buffer, next_line_start);
        self.cursor = byte_at_visual_col(&self.buffer, next_line_start, next_line_end, *target_col);
    }

    pub fn backspace(&mut self) {
        if let Some(prev) = prev_char_start(&self.buffer, self.cursor) {
            self.buffer.drain(prev..self.cursor);
            self.cursor = prev;
            self.desired_col = None;
        }
    }

    pub fn delete(&mut self) {
        if let Some(next) = next_char_end(&self.buffer, self.cursor) {
            self.buffer.drain(self.cursor..next);
            self.desired_col = None;
        }
    }

    pub fn delete_word_back(&mut self) {
        if self.cursor == 0 {
            return;
        }

        let mut start = self.cursor;

        while let Some(prev) = prev_char_start(&self.buffer, start) {
            let c = char_at(&self.buffer, prev);
            if !c.is_whitespace() {
                break;
            }
            start = prev;
        }

        while let Some(prev) = prev_char_start(&self.buffer, start) {
            let c = char_at(&self.buffer, prev);
            if c.is_whitespace() {
                break;
            }
            start = prev;
        }

        self.buffer.drain(start..self.cursor);
        self.cursor = start;
        self.desired_col = None;
    }

    pub fn insert_char(&mut self, c: char) {
        self.buffer.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.desired_col = None;
    }

    pub fn insert_str(&mut self, s: &str) {
        self.buffer.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.desired_col = None;
    }
}

fn prev_char_start(s: &str, idx: usize) -> Option<usize> {
    if idx == 0 || idx > s.len() {
        return None;
    }
    s[..idx].char_indices().last().map(|(i, _)| i)
}

fn next_char_end(s: &str, idx: usize) -> Option<usize> {
    if idx >= s.len() {
        return None;
    }
    s[idx..].chars().next().map(|c| idx + c.len_utf8())
}

fn char_at(s: &str, idx: usize) -> char {
    s[idx..].chars().next().unwrap_or('\0')
}

fn line_start(s: &str, line_idx: usize) -> usize {
    if line_idx == 0 {
        return 0;
    }

    let mut current_line = 0;
    for (idx, ch) in s.char_indices() {
        if ch == '\n' {
            current_line += 1;
            if current_line == line_idx {
                return idx + 1;
            }
        }
    }
    s.len()
}

fn line_end(s: &str, start: usize) -> usize {
    s[start..]
        .find('\n')
        .map(|offset| start + offset)
        .unwrap_or(s.len())
}

fn line_bounds_at(s: &str, idx: usize) -> (usize, usize) {
    let start = s[..idx].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let end = s[idx..].find('\n').map(|p| idx + p).unwrap_or(s.len());
    (start, end)
}

fn line_col_at(s: &str, idx: usize) -> (usize, usize) {
    let line = s[..idx].bytes().filter(|b| *b == b'\n').count();
    let line_start = s[..idx].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let col = s[line_start..idx].chars().count();
    (line, col)
}

fn byte_at_visual_col(s: &str, line_start: usize, line_end: usize, col: usize) -> usize {
    let mut byte = line_start;
    let mut current_col = 0;
    for ch in s[line_start..line_end].chars() {
        if current_col >= col {
            break;
        }
        byte += ch.len_utf8();
        current_col += 1;
    }
    byte
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEventKind, KeyEventState};

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    #[test]
    fn inserts_and_moves_cursor() {
        let mut ed = EditorState::new();
        ed.insert_str("hello");
        ed.move_left();
        ed.insert_char('X');
        assert_eq!(ed.buffer(), "hellXo");
        assert_eq!(ed.cursor(), 5);
    }

    #[test]
    fn alt_backspace_deletes_word_and_spaces() {
        let mut ed = EditorState::new();
        ed.insert_str("alpha  beta");
        ed.handle_key(key(KeyCode::Backspace, KeyModifiers::ALT));
        assert_eq!(ed.buffer(), "alpha  ");
        ed.handle_key(key(KeyCode::Backspace, KeyModifiers::ALT));
        assert_eq!(ed.buffer(), "");
    }

    #[test]
    fn ctrl_w_uses_word_delete() {
        let mut ed = EditorState::new();
        ed.insert_str("let value = 42");
        ed.handle_key(key(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert_eq!(ed.buffer(), "let value = ");
    }

    #[test]
    fn up_down_preserve_column_across_lines() {
        let mut ed = EditorState::new();
        ed.insert_str("abcd\nxy\n12345");
        ed.move_end();
        ed.move_up();
        assert_eq!(ed.line_col(), (1, 2));
        ed.move_up();
        assert_eq!(ed.line_col(), (0, 4));
        ed.move_down();
        assert_eq!(ed.line_col(), (1, 2));
        ed.move_down();
        assert_eq!(ed.line_col(), (2, 5));
    }

    #[test]
    fn handle_ctrl_s_exits() {
        let mut ed = EditorState::new();
        let action = ed.handle_key(key(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(action, EditorAction::SaveAndExit);
    }

    #[test]
    fn handle_ctrl_q_exits_cancel() {
        let mut ed = EditorState::new();
        let action = ed.handle_key(key(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert_eq!(action, EditorAction::Cancel);
    }

    #[test]
    fn handle_ctrl_z_exits_cancel() {
        let mut ed = EditorState::new();
        let action = ed.handle_key(key(KeyCode::Char('z'), KeyModifiers::CONTROL));
        assert_eq!(action, EditorAction::Cancel);
    }
}
