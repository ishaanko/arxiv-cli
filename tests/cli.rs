//! End-to-end checks that run offline. Every case here fails before any
//! network call (bad ID / empty query), so the suite never touches arxiv.org.

use std::process::Command;

fn arxiv() -> Command {
    Command::new(env!("CARGO_BIN_EXE_arxiv"))
}

/// With --json, a failure must still print valid JSON `{"error": ...}` on
/// stdout, a one-line error on stderr, and exit nonzero.
#[test]
fn json_error_contract_never_emits_empty_stdout() {
    let out = arxiv()
        .args(["get", "not-a-real-id", "--json"])
        .output()
        .expect("run arxiv");

    assert!(!out.status.success(), "must exit nonzero on failure");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.trim().is_empty(), "stdout must not be empty with --json");
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout must be valid JSON");
    assert!(
        parsed.get("error").and_then(|v| v.as_str()).is_some(),
        "JSON must carry a string error field, got: {stdout}"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("error"), "stderr must carry a message");
    assert_eq!(stderr.trim().lines().count(), 1, "stderr error is one line");
}

/// Without --json, a failure prints nothing to stdout (stays pipe-safe) and
/// exits nonzero with a message on stderr.
#[test]
fn plain_error_keeps_stdout_empty() {
    let out = arxiv()
        .args(["get", "not-a-real-id"])
        .output()
        .expect("run arxiv");

    assert!(!out.status.success());
    assert!(out.stdout.is_empty(), "stdout must be empty without --json");
    assert!(!out.stderr.is_empty(), "stderr must carry a message");
}

/// --json and --ids-only are two different output modes; asking for both would
/// make stdout ambiguous (bare IDs on success, JSON on error), so it is
/// rejected up front rather than silently picking one.
#[test]
fn json_and_ids_only_conflict() {
    let out = arxiv()
        .args(["search", "graph neural networks", "--json", "--ids-only"])
        .output()
        .expect("run arxiv");

    assert!(!out.status.success(), "conflicting flags must be rejected");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("cannot be used with") || stderr.contains("conflict"),
        "stderr should explain the conflict, got: {stderr}"
    );
}

/// An empty query is a usage error and must honor the JSON contract too.
#[test]
fn empty_query_json_error() {
    let out = arxiv()
        .args(["search", "", "--json"])
        .output()
        .expect("run arxiv");

    assert!(!out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout must be valid JSON");
    assert!(parsed.get("error").is_some());
}
