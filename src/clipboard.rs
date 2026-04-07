use anyhow::{anyhow, Context, Result};
use base64::Engine;
use std::io::{self, IsTerminal, Write};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy)]
pub struct ClipboardOutcome {
    pub provider: &'static str,
    pub verified: bool,
}

pub fn copy_to_clipboard(contents: &str) -> Result<ClipboardOutcome> {
    #[cfg(target_os = "windows")]
    {
        if clipboard_win::set_clipboard_string(contents).is_ok() {
            return Ok(ClipboardOutcome {
                provider: "clipboard-win",
                verified: true,
            });
        }
    }

    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        if clipboard.set_text(contents.to_owned()).is_ok() {
            return Ok(ClipboardOutcome {
                provider: "arboard",
                verified: true,
            });
        }
    }

    if let Some(outcome) = copy_with_platform_tools(contents)? {
        return Ok(outcome);
    }

    Err(anyhow!(
        "failed to copy to clipboard: no clipboard provider available"
    ))
}

fn copy_with_platform_tools(contents: &str) -> Result<Option<ClipboardOutcome>> {
    // iTerm2 exposes `it2copy` and it works well in remote/VM shells.
    if run_copy_cmd("it2copy", &[], contents)? {
        return Ok(Some(ClipboardOutcome {
            provider: "it2copy",
            verified: true,
        }));
    }

    #[cfg(target_os = "macos")]
    {
        if run_copy_cmd("pbcopy", &[], contents)? {
            return Ok(Some(ClipboardOutcome {
                provider: "pbcopy",
                verified: true,
            }));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if run_copy_cmd("wl-copy", &[], contents)? {
            return Ok(Some(ClipboardOutcome {
                provider: "wl-copy",
                verified: true,
            }));
        }
        if run_copy_cmd("xclip", &["-selection", "clipboard"], contents)? {
            return Ok(Some(ClipboardOutcome {
                provider: "xclip",
                verified: true,
            }));
        }
        if run_copy_cmd("xsel", &["--clipboard", "--input"], contents)? {
            return Ok(Some(ClipboardOutcome {
                provider: "xsel",
                verified: true,
            }));
        }
    }

    if copy_with_osc52(contents)? {
        return Ok(Some(ClipboardOutcome {
            provider: "osc52",
            verified: false,
        }));
    }

    Ok(None)
}

fn copy_with_osc52(contents: &str) -> Result<bool> {
    if !io::stdout().is_terminal() {
        return Ok(false);
    }

    let encoded = base64::engine::general_purpose::STANDARD.encode(contents.as_bytes());
    let sequence = if std::env::var_os("TMUX").is_some() {
        // tmux passthrough wrapper for OSC52.
        format!("\x1bPtmux;\x1b\x1b]52;c;{}\x07\x1b\\", encoded)
    } else {
        format!("\x1b]52;c;{}\x07", encoded)
    };

    let mut stdout = io::stdout();
    stdout
        .write_all(sequence.as_bytes())
        .context("failed to write OSC52 sequence")?;
    stdout.flush().context("failed to flush OSC52 sequence")?;
    Ok(true)
}

fn run_copy_cmd(cmd: &str, args: &[&str], contents: &str) -> Result<bool> {
    let mut child = match Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return Ok(false),
    };

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(contents.as_bytes())
            .with_context(|| format!("failed writing to {cmd} stdin"))?;
    }

    let status = child.wait().context("clipboard command failed")?;
    Ok(status.success())
}
