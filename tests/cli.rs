use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::process::Command as ProcessCommand;
use tempfile::tempdir;

fn binary() -> Command {
    Command::cargo_bin("risk-gate").expect("binary")
}

#[test]
fn cli_init_creates_permissive_commented_config() {
    let directory = tempdir().unwrap();
    binary()
        .args(["init", "--root"])
        .arg(directory.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("created"));
    let path = directory.path().join(".risk-gate.yaml");
    let contents = fs::read_to_string(&path).unwrap();
    assert!(contents.contains("path: ./"));
    assert!(contents.contains("minimal | low | normal | high | danger"));
    assert!(contents.contains("instantMerge: true"));
    assert!(contents.contains("newCodePolicy: include"));
    assert!(contents.contains("riskLabels: true"));
    let config = risk_gate::config::Config::load(path).unwrap();
    assert!(config.risk_labels);
}

#[test]
fn cli_init_does_not_overwrite_without_force() {
    let directory = tempdir().unwrap();
    let path = directory.path().join(".risk-gate.yaml");
    fs::write(&path, "keep").unwrap();
    binary()
        .args(["init", "--root"])
        .arg(directory.path())
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("--force"));
    assert_eq!(fs::read_to_string(path).unwrap(), "keep");
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
        .env_remove("GITHUB_BASE_SHA")
        .env_remove("GITHUB_SHA")
        .env_remove("GITHUB_EVENT_PATH")
        .env_remove("GITHUB_TOKEN")
        .env_remove("GITHUB_REPOSITORY")
        .env_remove("GITHUB_API_URL")
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
    let label = server
        .mock("GET", "/repos/o/r/labels/risk%3A%20high")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"name":"risk: high","color":"c2410c"}"#)
        .create();
    let current_labels = server
        .mock("GET", "/repos/o/r/issues/1/labels?per_page=100&page=1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .create();
    let add_label = server
        .mock("POST", "/repos/o/r/issues/1/labels")
        .match_body(mockito::Matcher::Json(serde_json::json!({"labels": ["risk: high"]})))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"name":"risk: high","color":"c2410c"}]"#)
        .create();
    let directory = github_mode_checkout("");
    let output = run_github_mode(&directory, &server.url()).success();
    output.stdout(predicate::str::contains("Risk Gate"));
    let written = fs::read_to_string(directory.path().join("output")).unwrap();
    assert!(written.contains("risk=high"));
    assert!(written.contains("risk_label=risk: high"));
    files.assert();
    comments.assert();
    create.assert();
    label.assert();
    current_labels.assert();
    add_label.assert();
}

#[test]
fn cli_github_mode_skips_labels_when_disabled() {
    let mut server = mockito::Server::new();
    let _report = github_report_mocks(&mut server);
    let no_labels = server
        .mock("GET", mockito::Matcher::Regex("labels".into()))
        .expect(0)
        .create();
    let directory = github_mode_checkout("riskLabels: false\n");
    run_github_mode(&directory, &server.url()).success();
    let written = fs::read_to_string(directory.path().join("output")).unwrap();
    assert!(written.contains("risk=high"));
    assert!(!written.contains("risk_label"));
    no_labels.assert();
}

#[test]
fn cli_github_mode_fails_when_label_cannot_be_applied() {
    let mut server = mockito::Server::new();
    let _report = github_report_mocks(&mut server);
    let forbidden = server
        .mock("GET", "/repos/o/r/labels/risk%3A%20high")
        .with_status(403)
        .with_body("Resource not accessible by integration")
        .create();
    let directory = github_mode_checkout("");
    run_github_mode(&directory, &server.url())
        .failure()
        .code(3)
        .stderr(predicate::str::contains("cannot apply risk label"));
    let written = fs::read_to_string(directory.path().join("output")).unwrap();
    assert!(written.contains("risk=high"));
    assert!(!written.contains("risk_label"));
    forbidden.assert();
}

fn github_report_mocks(server: &mut mockito::ServerGuard) -> Vec<mockito::Mock> {
    vec![
        server
            .mock("GET", "/repos/o/r/pulls/1/files?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"filename":"src/main.py","status":"modified"}]"#)
            .create(),
        server
            .mock("GET", "/repos/o/r/issues/1/comments?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create(),
        server
            .mock("POST", "/repos/o/r/issues/1/comments")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":3,"body":"report","html_url":"https://example.test/report"}"#)
            .create(),
    ]
}

fn github_mode_checkout(extra_config: &str) -> tempfile::TempDir {
    let directory = tempdir().unwrap();
    fs::create_dir(directory.path().join("src")).unwrap();
    fs::write(directory.path().join("src/main.py"), "print(1)\n").unwrap();
    fs::write(
        directory.path().join("risk.yaml"),
        format!("version: 1\nriskMap:\n  - path: src/\n    risk: high\n{extra_config}"),
    )
    .unwrap();
    fs::write(
        directory.path().join("event.json"),
        r#"{"number":1,"pull_request":{"base":{"sha":"base"},"head":{"sha":"head"}}}"#,
    )
    .unwrap();
    directory
}

fn run_github_mode(directory: &tempfile::TempDir, api_url: &str) -> assert_cmd::assert::Assert {
    binary()
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
            api_url,
        ])
        .env("GITHUB_OUTPUT", directory.path().join("output"))
        .current_dir(directory.path())
        .assert()
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
