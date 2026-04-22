//! Subprocess adapter for LaTeX → Unicode conversion via pylatexenc.
//!
//! The Python script is embedded in the binary and passed via `python3 -c`.
//! No external script file or env var required — only `python3` in PATH
//! and `pylatexenc` installed in the Python environment.

use hexforge::HexforgeError;
use serde::Deserialize;

use crate::logic::full_import::ConvertOutcome;
use crate::process::latex_columns::LatexBatchConverter;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::{Duration, timeout};

const SUBPROCESS_TIMEOUT: Duration = Duration::from_secs(60);
const CHUNK_SIZE: usize = 5_000;

/// Python script embedded at compile time.
/// Reads a JSON array of strings from stdin; writes a JSON array of
/// {status, result|message} items to stdout; exits non-zero on fatal errors.
const PYTHON_SCRIPT: &str = r#"
import json, sys

def preprocess_quotes(text):
    import re
    # TeX quote ligatures: replace before pylatexenc so they survive conversion.
    # Order matters: double first so single doesn't fire on the first char.
    text = text.replace("``", "\u201c")   # `` → "
    text = text.replace("''", "\u201d")   # '' → "
    # Only replace backtick when NOT preceded by \ (which is a grave accent command, e.g. \`e → è).
    text = re.sub(r"(?<!\\)`", "\u2018", text)   # ` → ' (but not \`)
    return text

def convert_one(converter, text):
    try:
        result = converter.latex_to_text(preprocess_quotes(text))
        return {"status": "ok", "result": " ".join(result.split())}
    except Exception as exc:
        return {"status": "error", "message": f"{type(exc).__name__}: {exc}"}

try:
    from pylatexenc.latex2text import LatexNodes2Text
    converter = LatexNodes2Text()
except ImportError as exc:
    sys.stderr.write(f"pylatexenc not available: {exc}\n")
    sys.exit(1)

try:
    texts = json.loads(sys.stdin.read())
except Exception as exc:
    sys.stderr.write(f"Failed to parse stdin: {exc}\n")
    sys.exit(1)

sys.stdout.write(json.dumps([convert_one(converter, t) for t in texts], ensure_ascii=False))
sys.stdout.write("\n")
"#;

/// Per-item result as returned by the Python script.
#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum PyConvertItem {
    Ok { result: String },
    Error { message: String },
}

/// Calls the embedded Python script to convert a batch of LaTeX strings.
///
/// Errors:
/// - `Err(HexforgeError)` — subprocess failed to start, timed out, crashed, or
///   returned unparseable output. The entire batch fails.
/// - `Ok(vec![..., PyConvertItem::Error { .. }, ...])` — individual items with
///   invalid LaTeX produce an error entry at the same position; the rest succeed.
pub struct PyLatexConverter;

impl PyLatexConverter {
    /// Convert a single chunk via one Python subprocess invocation.
    async fn convert_chunk(&self, texts: &[String]) -> Result<Vec<PyConvertItem>, HexforgeError> {
        let input =
            serde_json::to_vec(texts).map_err(|e| HexforgeError::internal(e.to_string()))?;

        let mut child = Command::new("python3")
            .args(["-c", PYTHON_SCRIPT])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| HexforgeError::internal(format!("Failed to spawn python3: {e}")))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&input).await.map_err(|e| {
                HexforgeError::internal(format!("Failed to write to converter stdin: {e}"))
            })?;
        }

        let output = timeout(SUBPROCESS_TIMEOUT, child.wait_with_output())
            .await
            .map_err(|_| {
                HexforgeError::internal(format!(
                    "LaTeX converter timed out after {}s",
                    SUBPROCESS_TIMEOUT.as_secs()
                ))
            })?
            .map_err(|e| HexforgeError::internal(format!("Converter wait error: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(HexforgeError::internal(format!(
                "LaTeX converter exited with {}: {stderr}",
                output.status
            )));
        }

        serde_json::from_slice(&output.stdout)
            .map_err(|e| HexforgeError::internal(format!("Failed to parse converter output: {e}")))
    }

    /// Convert texts in chunks of CHUNK_SIZE, concatenating all results.
    pub async fn convert_batch(
        &self,
        texts: &[String],
    ) -> Result<Vec<PyConvertItem>, HexforgeError> {
        let mut all = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(CHUNK_SIZE) {
            all.extend(self.convert_chunk(chunk).await?);
        }
        Ok(all)
    }
}

impl LatexBatchConverter for PyLatexConverter {
    async fn convert(&self, texts: Vec<String>) -> Result<Vec<ConvertOutcome>, HexforgeError> {
        let py_items = self.convert_batch(&texts).await?;
        let outcomes = texts
            .into_iter()
            .zip(py_items)
            .map(|(original, item)| match item {
                PyConvertItem::Ok { result } => ConvertOutcome::Ok(result),
                PyConvertItem::Error { message } => ConvertOutcome::Err { original, message },
            })
            .collect();
        Ok(outcomes)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::process::{Command, Stdio};

    /// Run preprocess_quotes in isolation (no pylatexenc needed) and return the result.
    fn preprocess(input: &str) -> String {
        let script = r#"
import re, json, sys

def preprocess_quotes(text):
    text = text.replace("``", "\u201c")
    text = text.replace("''", "\u201d")
    text = re.sub(r"(?<!\\)`", "\u2018", text)
    return text

data = json.loads(sys.stdin.read())
sys.stdout.write(json.dumps(preprocess_quotes(data)))
"#;
        let mut child = Command::new("python3")
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("python3 not found");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(serde_json::to_vec(input).unwrap().as_slice())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        serde_json::from_slice::<String>(&out.stdout).unwrap()
    }

    #[test]
    fn grave_accent_command_not_replaced() {
        // \`e is LaTeX grave accent → produces è. The backtick must survive preprocessing.
        let input = r"gen{\`e}se";
        let result = preprocess(input);
        assert!(
            result.contains("\\`"),
            "grave accent backtick was incorrectly replaced: {result}"
        );
        assert_eq!(
            result, input,
            "grave accent input should be unchanged by preprocess"
        );
    }

    #[test]
    fn standalone_backtick_becomes_open_quote() {
        // A bare ` not preceded by \ is a TeX open-quote ligature
        let result = preprocess("`word");
        assert_eq!(result, "\u{2018}word");
    }

    #[test]
    fn double_backtick_becomes_open_double_quote() {
        let result = preprocess("``word''");
        assert_eq!(result, "\u{201c}word\u{201d}");
    }

    #[test]
    fn grave_and_standalone_in_same_text() {
        // "G.E.~Moore et la gen{\`e}se" — the \`e must stay, other backticks replaced
        let input = r"G.E.~Moore et la gen{\`e}se de la philosophie analytique";
        let result = preprocess(input);
        assert!(
            result.contains("\\`e"),
            "\\`e was incorrectly replaced in: {result}"
        );
        // No standalone backtick in this input — string should be unchanged
        assert_eq!(result, input);
    }
}
