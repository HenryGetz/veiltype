use std::time::{SystemTime, UNIX_EPOCH};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::editor::EditorState;

#[derive(Debug, Clone, Copy)]
pub struct LanguageProfile {
    pub display_name: &'static str,
    pub extension: &'static str,
    pub file_name: &'static str,
    pub breadcrumb: &'static str,
    pub lsp: &'static str,
    pub decoy_code: &'static str,
}

const PROFILES: [LanguageProfile; 4] = [
    LanguageProfile {
        display_name: "Rust",
        extension: "rs",
        file_name: "session.rs",
        breadcrumb: "src > runtime > session.rs",
        lsp: "rust-analyzer",
        decoy_code: r#"use std::time::Duration;

pub async fn stream_snapshots(pool: &DbPool) -> anyhow::Result<()> {
    let mut batch = Vec::with_capacity(64);
    let mut tick = tokio::time::interval(Duration::from_millis(250));

    loop {
        tick.tick().await;
        while let Some(job) = pool.dequeue().await? {
            batch.push(job.into_snapshot());
            if batch.len() >= 64 {
                persist_batch(&batch).await?;
                batch.clear();
            }
        }
    }
}
"#,
    },
    LanguageProfile {
        display_name: "TypeScript",
        extension: "ts",
        file_name: "pipeline.ts",
        breadcrumb: "packages > app > src > pipeline.ts",
        lsp: "tsserver",
        decoy_code: r#"type Frame = { id: string; level: "info" | "warn" | "error" };

export async function flushFrames(frames: Frame[]) {
  const byLevel = new Map<string, number>();
  for (const frame of frames) {
    byLevel.set(frame.level, (byLevel.get(frame.level) ?? 0) + 1);
  }

  await fetch("/api/frames", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ frames, summary: Object.fromEntries(byLevel) }),
  });
}
"#,
    },
    LanguageProfile {
        display_name: "Python",
        extension: "py",
        file_name: "collector.py",
        breadcrumb: "services > jobs > collector.py",
        lsp: "pyright",
        decoy_code: r#"from dataclasses import dataclass
from datetime import datetime


@dataclass
class Record:
    job_id: str
    created_at: datetime
    payload: dict


def collect(records: list[Record]) -> dict[str, int]:
    totals: dict[str, int] = {"ok": 0, "failed": 0}
    for record in records:
        key = "ok" if record.payload.get("status") == "ok" else "failed"
        totals[key] += 1
    return totals
"#,
    },
    LanguageProfile {
        display_name: "Go",
        extension: "go",
        file_name: "scheduler.go",
        breadcrumb: "cmd > worker > scheduler.go",
        lsp: "gopls",
        decoy_code: r#"package worker

import "time"

type Scheduler struct {
    every time.Duration
    run   func() error
}

func (s *Scheduler) Start(stop <-chan struct{}) {
    ticker := time.NewTicker(s.every)
    defer ticker.Stop()

    for {
        select {
        case <-stop:
            return
        case <-ticker.C:
            _ = s.run()
        }
    }
}
"#,
    },
];

#[derive(Debug, Clone, Copy)]
pub enum ProfileKind {
    Rust,
    TypeScript,
    Python,
    Go,
}

#[derive(Debug, Clone)]
pub struct FakeTyper {
    profile: LanguageProfile,
    tape: Vec<char>,
    idx: usize,
    prng: u64,
}

impl FakeTyper {
    pub fn new() -> Self {
        Self::with_profile(None)
    }

    pub fn with_profile(kind: Option<ProfileKind>) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0xC0FFEE_u64);
        let profile = match kind {
            Some(ProfileKind::Rust) => PROFILES[0],
            Some(ProfileKind::TypeScript) => PROFILES[1],
            Some(ProfileKind::Python) => PROFILES[2],
            Some(ProfileKind::Go) => PROFILES[3],
            None => PROFILES[(nanos as usize) % PROFILES.len()],
        };
        let tape: Vec<char> = profile.decoy_code.chars().filter(|c| *c != '\r').collect();
        Self {
            profile,
            tape,
            idx: 0,
            prng: nanos ^ 0xA5A5_5A5A_1337_4242,
        }
    }

    pub fn profile(&self) -> LanguageProfile {
        self.profile
    }

    pub fn apply_to_decoy(&mut self, key: KeyEvent, decoy: &mut EditorState) {
        if !is_visible_key(key) {
            return;
        }

        let chars_to_emit = self.next_cadence();
        for _ in 0..chars_to_emit {
            decoy.insert_char(self.next_visible_char());
        }
    }

    pub fn apply_paste_to_decoy(&mut self, pasted: &str, decoy: &mut EditorState) {
        for ch in pasted.chars() {
            if ch == '\u{7f}' || ch == '\u{8}' {
                continue;
            }
            // Ignore control characters in pasted text; only visible chars advance decoy typing.
            if ch.is_control() && ch != '\n' && ch != '\t' {
                continue;
            }
            let chars_to_emit = self.next_cadence();
            for _ in 0..chars_to_emit {
                decoy.insert_char(self.next_visible_char());
            }
        }
    }

    fn next_cadence(&mut self) -> usize {
        let roll = (self.next_rand() % 100) as usize;
        if roll < 10 {
            0
        } else if roll < 82 {
            1
        } else {
            2
        }
    }

    fn next_visible_char(&mut self) -> char {
        if self.tape.is_empty() {
            return ' ';
        }

        let ch = self.tape[self.idx % self.tape.len()];
        self.idx = (self.idx + 1) % self.tape.len();
        ch
    }

    fn next_rand(&mut self) -> u64 {
        self.prng = self
            .prng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        self.prng
    }
}

impl Default for FakeTyper {
    fn default() -> Self {
        Self::new()
    }
}

fn is_visible_key(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char(_))
        && !key.modifiers.contains(KeyModifiers::CONTROL)
        && !key.modifiers.contains(KeyModifiers::SUPER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};

    #[test]
    fn emits_decoy_for_char_keys() {
        let mut fake = FakeTyper::new();
        let mut decoy = EditorState::new();
        let input = KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };

        for _ in 0..5 {
            fake.apply_to_decoy(input, &mut decoy);
        }
        assert!(!decoy.buffer().is_empty());
        assert!(!decoy.buffer().contains('x'));
    }

    #[test]
    fn keeps_ctrl_shortcuts_hidden() {
        let mut fake = FakeTyper::new();
        let mut decoy = EditorState::new();
        let input = KeyEvent {
            code: KeyCode::Char('w'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        fake.apply_to_decoy(input, &mut decoy);
        assert!(decoy.buffer().is_empty());
    }

    #[test]
    fn ignores_backspace_and_enter() {
        let mut fake = FakeTyper::new();
        let mut decoy = EditorState::new();
        let enter = KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        let backspace = KeyEvent {
            code: KeyCode::Backspace,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };

        fake.apply_to_decoy(enter, &mut decoy);
        fake.apply_to_decoy(backspace, &mut decoy);
        assert!(decoy.buffer().is_empty());
    }

    #[test]
    fn paste_advances_decoy() {
        let mut fake = FakeTyper::new();
        let mut decoy = EditorState::new();
        fake.apply_paste_to_decoy("hello\nworld", &mut decoy);
        assert!(!decoy.buffer().is_empty());
    }
}
