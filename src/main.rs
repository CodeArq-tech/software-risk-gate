use clap::Parser;
use risk_gate::config::Config;
use risk_gate::diff::GitDiffProvider;
use risk_gate::engine::{
    analyze, analyze_ast_impact, analyze_risk_only, analyze_with_approval, plan_policy,
};
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
    #[arg(
        long,
        conflicts_with = "dry_run",
        help = "Show AST changes and dependency impact without requiring configuration"
    )]
    ast_impact: bool,
}

fn main() {
    if is_init_command() {
        let cli = InitCli::parse_from(init_arguments());
        if let Err(error) = run_init(&cli) {
            fail(error, 2);
        }
        return;
    }
    let cli = Cli::parse();
    let _ = telemetry::event(
        "analysis_started",
        &[("config", cli.config.display().to_string())],
    );
    if cli.ast_impact {
        if let Err(error) = run_ast_impact(&cli) {
            fail(error, 3);
        }
        return;
    }
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

fn is_init_command() -> bool {
    env::args().nth(1).as_deref() == Some("init")
}

fn init_arguments() -> Vec<String> {
    let mut args = vec!["risk-gate init".to_owned()];
    args.extend(env::args().skip(2));
    args
}

#[derive(Debug, Parser)]
#[command(
    name = "risk-gate init",
    about = "Create a permissive risk-gate configuration"
)]
struct InitCli {
    #[arg(long, default_value = ".", help = "Repository directory")]
    root: PathBuf,
    #[arg(
        long,
        default_value = ".risk-gate.yaml",
        help = "Configuration file path, relative to --root"
    )]
    output: PathBuf,
    #[arg(long, help = "Replace an existing configuration file")]
    force: bool,
}

fn run_init(cli: &InitCli) -> Result<(), String> {
    let output = cli.root.join(&cli.output);
    if output.exists() && !cli.force {
        return Err(format!(
            "{} already exists; use --force to replace it",
            output.display()
        ));
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&output, default_config_template()).map_err(|error| error.to_string())?;
    println!("created {}", output.display());
    Ok(())
}

fn default_config_template() -> &'static str {
    r#"# Risk Gate configuration (version 1)
# The generated values are intentionally permissive: every risk level allows an
# instant merge, and no validation or approval is required until you customize it.
version: 1

riskMap:
  - path: ./                         # path: repository-relative path; ./ matches the whole repository
    risk: minimal                    # minimal | low | normal | high | danger (higher wins)
    description: "Default repository rule" # text shown in the pull-request report
    newCodePolicy: include           # include | ignore; ignore isolated, additive new code

ignorePaths:                         # optional paths excluded from risk and graph analysis
  # - tests/                          # directory prefix; ignored files are not risk evidence
  # - generated/                      # use / and repository-relative paths

gateRules:
  minimal:
    instantMerge: true               # true | false; true reports the risk without blocking the merge
  low:
    instantMerge: true               # true | false; validations below still run when configured
  normal:
    instantMerge: true               # true | false; set false to block when gates fail
  high:
    instantMerge: true               # true | false; optionally add requireApprovalGroup
  danger:
    instantMerge: true               # true | false; optionally add validations and approval
    # requireApprovalGroup: architects # name from approvalGroups; requires team/users approval
    # validations: [e2e, mutationTesting] # every listed validation must return success

validations:
  # e2e:
  #   command: "./scripts/e2e.sh"    # shell command executed from the repository root
  #   timeoutSeconds: 1800            # positive timeout in seconds
  # mutationTesting: "./scripts/mutation.sh" # shorthand command form

approvalGroups:
  # architects:
  #   team: architects                # GitHub team slug, or use users below
  #   users: [alice, bob]              # GitHub login names; one approval is enough

limits:
  maxFiles: 10000                     # positive AST file limit
  maxAstNodes: 1000000                # positive AST node limit
  maxCommentBytes: 60000              # maximum report/comment size in bytes
"#
}

fn run_ast_impact(cli: &Cli) -> Result<(), String> {
    let repository = cli
        .repository
        .clone()
        .ok_or("--repository is required with --ast-impact")?;
    let token = cli
        .token
        .clone()
        .ok_or("--token or GITHUB_TOKEN is required with --ast-impact")?;
    let pull_number = cli
        .pull_number
        .ok_or("--pull-number is required with --ast-impact")?;
    let mut api =
        GithubApi::new(&token, &repository, pull_number).map_err(|error| error.to_string())?;
    if let Some(api_url) = &cli.api_url {
        api = api.with_base_url(api_url);
    }
    let result = analyze_ast_impact(&cli.root, &api).map_err(|error| error.to_string())?;
    let output = AstImpactOutput {
        mode: "ast-impact",
        repository,
        pull_number,
        result,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output).map_err(|error| error.to_string())?
    );
    Ok(())
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

#[derive(Debug, serde::Serialize)]
struct AstImpactOutput {
    mode: &'static str,
    repository: String,
    #[serde(rename = "pullNumber")]
    pull_number: u64,
    result: risk_gate::engine::AstImpactResult,
}
