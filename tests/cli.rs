use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::process::Command as ProcessCommand;
use tempfile::tempdir;

fn binary() -> Command {
    Command::cargo_bin("risk-gate").expect("binary")
}

#[test]
fn cli_loads_configuration_without_diff_context() {
    let directory = tempdir().unwrap();
    fs::write(
        directory.path().join("risk.yaml"),
        "version: 1\nriskMap:\n  - path: src/\n    risk: normal\n",
    )
    .unwrap();
    binary()
        .arg("--config")
        .arg(directory.path().join("risk.yaml"))
        .assert()
        .success()
        .stdout(predicate::str::contains("loaded 1 risk rules"));
}

#[test]
fn cli_analyzes_local_git_range_and_returns_blocking_failure() {
    let directory = tempdir().unwrap();
    for args in [
        &["init"][..],
        &["config", "user.email", "test@example.test"][..],
        &["config", "user.name", "Test"][..],
    ] {
        assert!(
            ProcessCommand::new("git")
                .current_dir(directory.path())
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
    fs::write(directory.path().join("risk.yaml"), "version: 1\nriskMap:\n  - path: src/\n    risk: danger\ngateRules:\n  danger:\n    validations: [check]\nvalidations:\n  check:\n    command: 'false'\n").unwrap();
    fs::create_dir(directory.path().join("src")).unwrap();
    fs::write(directory.path().join("src/main.py"), "print(1)\n").unwrap();
    assert!(
        ProcessCommand::new("git")
            .current_dir(directory.path())
            .args(["add", "."])
            .status()
            .unwrap()
            .success()
    );
    assert!(
        ProcessCommand::new("git")
            .current_dir(directory.path())
            .args(["commit", "-m", "initial"])
            .status()
            .unwrap()
            .success()
    );
    fs::write(directory.path().join("src/main.py"), "print(2)\n").unwrap();
    let output = binary()
        .args([
            "--config",
            "risk.yaml",
            "--root",
            ".",
            "--base",
            "HEAD",
            "--head",
            "HEAD",
        ])
        .current_dir(directory.path())
        .assert();
    output
        .failure()
        .code(3)
        .stderr(predicate::str::contains("risk-gate"));
}

#[test]
fn cli_rejects_invalid_configuration() {
    let directory = tempdir().unwrap();
    fs::write(
        directory.path().join("risk.yaml"),
        "version: 2\nriskMap: []\n",
    )
    .unwrap();
    binary()
        .arg("--config")
        .arg(directory.path().join("risk.yaml"))
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("version"));
}

#[test]
fn cli_runs_github_mode_and_writes_outputs() {
    let mut server = mockito::Server::new();
    let files = server
        .mock("GET", "/repos/o/r/pulls/1/files?per_page=100&page=1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"filename":"src/main.py","status":"modified"}]"#)
        .create();
    let comments = server
        .mock("GET", "/repos/o/r/issues/1/comments?per_page=100&page=1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .create();
    let create = server
        .mock("POST", "/repos/o/r/issues/1/comments")
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body(r#"{"id":3,"body":"report","html_url":"https://example.test/report"}"#)
        .create();
    let directory = tempdir().unwrap();
    fs::create_dir(directory.path().join("src")).unwrap();
    fs::write(directory.path().join("src/main.py"), "print(1)\n").unwrap();
    fs::write(
        directory.path().join("risk.yaml"),
        "version: 1\nriskMap:\n  - path: src/\n    risk: high\n",
    )
    .unwrap();
    let event = directory.path().join("event.json");
    fs::write(
        &event,
        r#"{"number":1,"pull_request":{"base":{"sha":"base"},"head":{"sha":"head"}}}"#,
    )
    .unwrap();
    let output = binary()
        .args([
            "--config",
            "risk.yaml",
            "--root",
            ".",
            "--event-path",
            "event.json",
            "--token",
            "token",
            "--repository",
            "o/r",
            "--api-url",
            &server.url(),
        ])
        .env("GITHUB_OUTPUT", directory.path().join("output"))
        .current_dir(directory.path())
        .assert()
        .success();
    output.stdout(predicate::str::contains("Risk Gate"));
    let written = fs::read_to_string(directory.path().join("output")).unwrap();
    assert!(written.contains("risk=high"));
    files.assert();
    comments.assert();
    create.assert();
}

#[test]
fn cli_dry_run_returns_selected_policy_without_side_effects() {
    let mut server = mockito::Server::new();
    let files = server
        .mock("GET", "/repos/o/r/pulls/7/files?per_page=100&page=1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"filename":"src/main.py","status":"modified"}]"#)
        .create();
    let directory = tempdir().unwrap();
    fs::create_dir(directory.path().join("src")).unwrap();
    fs::write(directory.path().join("src/main.py"), "print(1)\n").unwrap();
    fs::write(
        directory.path().join("risk.yaml"),
        "version: 1\nriskMap:\n  - path: src/\n    risk: high\ngateRules:\n  high:\n    instantMerge: false\n    requireApprovalGroup: architects\n    validations: [e2e]\napprovalGroups:\n  architects:\n    users: [alice]\nvalidations:\n  e2e:\n    command: 'touch should-not-run'\n    timeoutSeconds: 42\n",
    )
    .unwrap();

    binary()
        .args([
            "--dry-run",
            "--config",
            "risk.yaml",
            "--root",
            ".",
            "--token",
            "token",
            "--repository",
            "o/r",
            "--pull-number",
            "7",
            "--api-url",
            &server.url(),
        ])
        .current_dir(directory.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"mode\": \"dry-run\""))
        .stdout(predicate::str::contains("\"risk\": \"high\""))
        .stdout(predicate::str::contains(
            "\"requireApprovalGroup\": \"architects\"",
        ))
        .stdout(predicate::str::contains("\"timeoutSeconds\": 42"));
    assert!(!directory.path().join("should-not-run").exists());
    files.assert();
}

#[test]
fn cli_ast_impact_works_without_risk_configuration() {
    let mut server = mockito::Server::new();
    let files = server
        .mock("GET", "/repos/o/r/pulls/8/files?per_page=100&page=1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"filename":"src/main.py","status":"modified"}]"#)
        .create();
    let directory = tempdir().unwrap();
    fs::create_dir_all(directory.path().join("src/payment")).unwrap();
    fs::write(
        directory.path().join("src/main.py"),
        "from .payment.core import authorize\n",
    )
    .unwrap();
    fs::write(
        directory.path().join("src/payment/core.py"),
        "def authorize():\n    return True\n",
    )
    .unwrap();

    let output = binary()
        .args([
            "--ast-impact",
            "--root",
            ".",
            "--token",
            "token",
            "--repository",
            "o/r",
            "--pull-number",
            "8",
            "--api-url",
            &server.url(),
        ])
        .current_dir(directory.path())
        .assert()
        .success();
    output
        .stdout(predicate::str::contains("\"mode\": \"ast-impact\""))
        .stdout(predicate::str::contains("\"changedFiles\""))
        .stdout(predicate::str::contains("\"authorize\""))
        .stdout(predicate::str::contains("\".payment.core\""))
        .stdout(predicate::str::contains("src/payment/core.py"));
    files.assert();
}
