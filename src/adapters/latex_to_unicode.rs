//! Subprocess adapter for LaTeX → Unicode conversion via pylatexenc.
//!
//! The Python script is embedded in the binary and passed via `python3 -c`.
//! No external script file or env var required — only `python3` in PATH
//! and `pylatexenc` installed in the Python environment.

use hexforge::HexforgeError;
use serde::Deserialize;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::{Duration, timeout};

/// Timeout for the entire subprocess call (all texts in the batch).
const SUBPROCESS_TIMEOUT: Duration = Duration::from_secs(30);

/// Python script embedded at compile time.
/// Reads a JSON array of strings from stdin; writes a JSON array of
/// {status, result|message} items to stdout; exits non-zero on fatal errors.
const PYTHON_SCRIPT: &str = r#"
import json, sys

def convert_one(converter, text):
    try:
        result = converter.latex_to_text(text)
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
    pub async fn convert_batch(
        &self,
        texts: &[String],
    ) -> Result<Vec<PyConvertItem>, HexforgeError> {
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
}
