use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::Result;

/// Snapshot of the shell environment we send to the LLM. History is read
/// lazily at prompt-build time via [`read_history`], so this struct stays
/// cheap to construct and reflects the freshest history on every call.
#[derive(Debug, Clone)]
pub struct ShellContext {
    pub kind: ShellKind,
    pub cwd: Option<PathBuf>,
    pub os: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    Unknown,
}

impl ShellKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::Unknown => "sh",
        }
    }

    fn detect() -> Self {
        let Ok(shell) = std::env::var("SHELL") else {
            return Self::Unknown;
        };
        let name = shell.rsplit('/').next().unwrap_or("").to_ascii_lowercase();
        match name.as_str() {
            "bash" => Self::Bash,
            "zsh" => Self::Zsh,
            "fish" => Self::Fish,
            _ => Self::Unknown,
        }
    }
}

impl ShellContext {
    /// Build a snapshot of the current shell environment.
    pub fn detect() -> Result<Self> {
        Ok(Self {
            kind: ShellKind::detect(),
            cwd: std::env::current_dir().ok(),
            os: std::env::consts::OS,
        })
    }
}

/// Best-effort history read. Returns the last `limit` lines from the shell's
/// history file, or an empty vec if the file can't be found / read. Reading
/// is done by seeking from the end of the file, so the cost is bounded by
/// `limit` rather than total file length.
///
/// Called at prompt-build time so the freshest history is captured on every
/// invocation, even if the user just ran a command.
pub fn read_history(kind: ShellKind, limit: usize) -> Vec<String> {
    let Ok(path) = history_path(kind) else {
        return Vec::new();
    };
    let Ok(raw) = read_tail_lines(&path, limit) else {
        return Vec::new();
    };
    match kind {
        ShellKind::Zsh => raw
            .lines()
            .map(|l| l.splitn(2, ';').nth(1).unwrap_or(l).to_string())
            .collect(),
        _ => raw.lines().map(str::to_string).collect(),
    }
}

/// Read the last `limit` newline-terminated lines from a file without loading
/// the whole thing. Walks backward in fixed-size chunks counting `\n` bytes,
/// then decodes (lossily) once we've collected enough.
fn read_tail_lines(path: &Path, limit: usize) -> Result<String> {
    const CHUNK: usize = 8 * 1024;

    if limit == 0 {
        return Ok(String::new());
    }

    let mut file = File::open(path)?;
    let total = file.seek(SeekFrom::End(0))?;
    if total == 0 {
        return Ok(String::new());
    }

    let mut remaining = total;
    let mut buf: Vec<u8> = Vec::new();
    let mut newlines = 0usize;
    // Count one extra `\n` so we land on the start of a line, not midway.
    let target = limit + 1;

    while remaining > 0 && newlines < target {
        let read_size = std::cmp::min(CHUNK as u64, remaining);
        remaining -= read_size;
        file.seek(SeekFrom::Start(remaining))?;

        let mut chunk = vec![0u8; read_size as usize];
        file.read_exact(&mut chunk)?;
        newlines += bytecount_newlines(&chunk);
        // Prepend so `buf` mirrors original file order.
        chunk.extend_from_slice(&buf);
        buf = chunk;
    }

    let text = String::from_utf8_lossy(&buf).into_owned();
    // Keep exactly the last `limit` newline-terminated (or trailing-no-newline)
    // lines so the function's contract matches its name.
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(limit);
    let mut out = lines[start..].join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

fn bytecount_newlines(buf: &[u8]) -> usize {
    buf.iter().filter(|&&b| b == b'\n').count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(contents: &str) -> tempfile_path::TempPath {
        tempfile_path::write(contents)
    }

    #[test]
    fn tail_reads_last_n_lines() {
        let body: String = (0..5000).map(|i| format!("line {i}\n")).collect();
        let path = write_tmp(&body);
        let tail = read_tail_lines(path.as_path(), 10).unwrap();
        let lines: Vec<&str> = tail.lines().collect();
        assert_eq!(lines.len(), 10);
        assert_eq!(lines.first().copied(), Some("line 4990"));
        assert_eq!(lines.last().copied(), Some("line 4999"));
    }

    #[test]
    fn tail_smaller_than_limit_returns_all() {
        let path = write_tmp("a\nb\nc\n");
        let tail = read_tail_lines(path.as_path(), 100).unwrap();
        assert_eq!(tail.lines().collect::<Vec<_>>(), vec!["a", "b", "c"]);
    }

    #[test]
    fn tail_zero_limit_returns_empty() {
        let path = write_tmp("a\nb\n");
        assert_eq!(read_tail_lines(path.as_path(), 0).unwrap(), "");
    }

    #[test]
    fn tail_handles_no_trailing_newline() {
        let path = write_tmp("alpha\nbeta\ngamma");
        let tail = read_tail_lines(path.as_path(), 2).unwrap();
        let lines: Vec<&str> = tail.lines().collect();
        assert_eq!(lines, vec!["beta", "gamma"]);
    }

    #[test]
    fn tail_handles_chunk_boundary() {
        // Force many small reads by writing more than CHUNK bytes.
        let big_line = "x".repeat(64);
        let body: String = (0..2000).map(|i| format!("{i}-{big_line}\n")).collect();
        let path = write_tmp(&body);
        let tail = read_tail_lines(path.as_path(), 3).unwrap();
        let lines: Vec<&str> = tail.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[2].starts_with("1999-"));
    }

    /// Tiny inline helper so we don't pull in the `tempfile` crate.
    mod tempfile_path {
        use std::io::Write;
        use std::path::{Path, PathBuf};

        pub struct TempPath(PathBuf);
        impl TempPath {
            pub fn as_path(&self) -> &Path {
                &self.0
            }
        }
        impl Drop for TempPath {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }

        pub fn write(contents: &str) -> TempPath {
            let mut dir = std::env::temp_dir();
            let name = format!(
                "interpreter-shell-test-{}-{}",
                std::process::id(),
                fastrand_like()
            );
            dir.push(name);
            let mut f = std::fs::File::create(&dir).expect("create tmp");
            f.write_all(contents.as_bytes()).expect("write tmp");
            TempPath(dir)
        }

        fn fastrand_like() -> u64 {
            use std::time::{SystemTime, UNIX_EPOCH};
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as u64)
                .unwrap_or(0);
            // Mix in a counter so adjacent calls don't collide.
            static COUNTER: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            nanos.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(c)
        }
    }

    // Silence dead-code warning for the helper re-export above.
    #[allow(dead_code)]
    fn _force_use(_: &dyn Write) {}
}

fn history_path(kind: ShellKind) -> Result<PathBuf> {
    let home = std::env::var("HOME").map(PathBuf::from)?;
    let p = match kind {
        ShellKind::Bash => home.join(".bash_history"),
        ShellKind::Zsh => std::env::var("HISTFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".zsh_history")),
        ShellKind::Fish => home.join(".local/share/fish/fish_history"),
        ShellKind::Unknown => home.join(".sh_history"),
    };
    Ok(p)
}
