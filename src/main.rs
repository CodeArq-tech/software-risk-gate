use clap::Parser;
use risk_gate::config::Config;
use risk_gate::diff::GitDiffProvider;
use risk_gate::engine::{analyze, analyze_risk_only, analyze_with_approval, plan_policy};
use risk_gate::gates::ShellExecutor;
use risk_gate::github::GithubApi;
use risk_gate::telemetry;
use std::path::PathBuf;
use std::{env, fs};

#[derive(Debug, Parser)]
#[command(name = "risk-gate", version, about = "Analyze pull-request risk")]
struct Cli {
    #[arg(long, default_value = ".risk-gate.yaml", env = "RISK_GATE_CONFIG")]
    config: PathBuf,
    #[arg(long, default_value = ".")]
    root: PathBuf,
    #[arg(long, env = "GITHUB_BASE_SHA")]
    base: Option<String>,
    #[arg(long, env = "GITHUB_SHA")]
    head: Option<String>,
    #[arg(long, env = "GITHUB_EVENT_PATH")]
    event_path: Option<PathBuf>,
    #[arg(long, env = "GITHUB_TOKEN")]
    token: Option<String>,
    #[arg(long, env = "GITHUB_REPOSITORY")]
    repository: Option<String>,
    #[arg(long, env = "GITHUB_API_URL")]
    api_url: Option<String>,
    #[arg(long)]
    pull_number: Option<u64>,
    #[arg(
        long,
        help = "Analyze a pull request without comments, approvals, or validations"
    )]
    dry_run: bool,
}

fn main() {
    let cli = Cli::parse();
    let _ = telemetry::event(
        "analysis_started",
        &[("config", cli.config.display().to_string())],
    );
    match Config::load(&cli.config) {
        Ok(config) => {
            if cli.dry_run {
                if let Err(error) = run_dry_run(&cli, &config) {
                    fail(error, 3);
                }
            } else if let (Some(base), Some(head)) = (cli.base.clone(), cli.head.clone()) {
                let diff = GitDiffProvider::new(&cli.root, base, head);
                match analyze(&config, &cli.root, &diff, &ShellExecutor::new(&cli.root)) {
                    Ok(result) => {
                        let _ = telemetry::event(
                            "analysis_finished",
                            &[
                                ("risk", result.assessment.risk.to_string()),
                                ("status", result.status.to_string()),
                            ],
                        );
                        println!("{}", result.report);
                        if result.gate.merge_blocked {
                            std::process::exit(1);
                        }
                    }
                    Err(error) => fail(error.to_string(), 3),
                }
            } else if cli.event_path.is_some() {
                if let Err(error) = run_github(&cli, &config) {
                    fail(error, 3);
                }
            } else {
                println!(
                    "loaded {} risk rules; provide --base and --head or a pull_request event",
                    config.risk_map.len()
                );
            }
        }
        Err(error) => {
            eprintln!("risk-gate: {error}");
            std::process::exit(2);
        }
    }
}

fn run_dry_run(cli: &Cli, config: &Config) -> Result<(), String> {
    let repository = cli
        .repository
        .clone()
        .ok_or("--repository is required with --dry-run")?;
    let token = cli
        .token
        .clone()
        .ok_or("--token or GITHUB_TOKEN is required with --dry-run")?;
    let pull_number = cli
        .pull_number
        .ok_or("--pull-number is required with --dry-run")?;
    let mut api =
        GithubApi::new(&token, &repository, pull_number).map_err(|error| error.to_string())?;
    if let Some(api_url) = &cli.api_url {
        api = api.with_base_url(api_url);
    }
    let result = analyze_risk_only(config, &cli.root, &api).map_err(|error| error.to_string())?;
    let risk = result.assessment.risk;
    let policy = plan_policy(config, risk);
    let output = DryRunOutput {
        mode: "dry-run",
        repository,
        pull_number,
        risk,
        assessment: result.assessment,
        changed_files: result.changed.len(),
        partial_files: result
            .parsed_files
            .iter()
            .filter(|file| file.error.is_some())
            .count(),
        policy,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn run_github(cli: &Cli, config: &Config) -> Result<(), String> {
    let event_path = cli
        .event_path
        .as_ref()
        .ok_or("GITHUB_EVENT_PATH is required for GitHub mode")?;
    let event: PullRequestEvent =
        serde_json::from_str(&fs::read_to_string(event_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let repository = cli
        .repository
        .clone()
        .ok_or("GITHUB_REPOSITORY is required for GitHub mode")?;
    let token = cli
        .token
        .clone()
        .ok_or("GITHUB_TOKEN is required for GitHub mode")?;
    let pull_number = cli.pull_number.unwrap_or(event.number);
    let mut api =
        GithubApi::new(&token, repository, pull_number).map_err(|error| error.to_string())?;
    if let Some(api_url) = &cli.api_url {
        api = api.with_base_url(api_url);
    }
    let result = analyze_with_approval(
        config,
        &cli.root,
        &api,
        &ShellExecutor::new(&cli.root),
        Some(&api),
    )
    .map_err(|error| error.to_string())?;
    let comment = api
        .upsert_comment(&result.report)
        .map_err(|error| error.to_string())?;
    write_output("risk", &result.assessment.risk.to_string());
    write_output("status", &result.status.to_string());
    write_output("merge_blocked", &result.gate.merge_blocked.to_string());
    if let Some(url) = &comment.url {
        write_output("report_url", url);
    }
    println!("{}", result.report);
    if result.gate.merge_blocked {
        return Err("configured gates blocked merge".into());
    }
    Ok(())
}

fn write_output(name: &str, value: &str) {
    if let Ok(path) = env::var("GITHUB_OUTPUT")
        && let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path)
    {
        use std::io::Write;
        let _ = writeln!(file, "{name}={value}");
    }
}

fn fail(message: String, code: i32) -> ! {
    let _ = telemetry::event(
        "analysis_failed",
        &[
            ("message", message.clone()),
            ("exit_code", code.to_string()),
        ],
    );
    eprintln!("risk-gate: {message}");
    std::process::exit(code);
}

#[derive(Debug, serde::Deserialize)]
struct PullRequestEvent {
    number: u64,
    #[serde(rename = "pull_request")]
    _pull_request: serde_json::Value,
}

#[derive(Debug, serde::Serialize)]
struct DryRunOutput {
    mode: &'static str,
    repository: String,
    #[serde(rename = "pullNumber")]
    pull_number: u64,
    risk: risk_gate::domain::RiskLevel,
    assessment: risk_gate::risk::RiskAssessment,
    #[serde(rename = "changedFiles")]
    changed_files: usize,
    #[serde(rename = "partialFiles")]
    partial_files: usize,
    policy: risk_gate::engine::PlannedPolicy,
}
