//! Prompt → command evaluation harness.
//!
//! These tests hit a real LLM, so they are `#[ignore]`d by default. Run them
//! explicitly with:
//!
//!     cargo test -p interpreter --test eval -- --ignored
//!
//! Override the model under test with `INTERPRETER_EVAL_MODEL=provider/model`.
//!
//! Each case is a (prompt, accepted-outputs) pair. The harness accepts an
//! exact match against any of the accepted outputs after normalizing
//! whitespace. This is intentionally strict for now; we'll replace the matcher
//! with a richer evaluator (semantic equivalence, dry-run execution, judge
//! model) once we formalize the eval pipeline.

use interpreter::config::Config;
use interpreter::llm_client;
use interpreter::shell::{ShellContext, ShellKind};

struct Case {
    prompt: &'static str,
    accept: &'static [&'static str],
}

const CASES: &[Case] = &[
    Case {
        prompt: "list the files in this directory",
        accept: &["ls"],
    },
    Case {
        prompt: "list files including hidden",
        accept: &["ls -a", "ls -A"],
    },
    Case {
        prompt: "show the current working directory",
        accept: &["pwd"],
    },
    Case {
        prompt: "print the contents of README.md",
        accept: &["cat README.md"],
    },
    Case {
        prompt: "count the number of lines in main.rs",
        accept: &["wc -l main.rs", "wc -l < main.rs"],
    },
    Case {
        prompt: "find all rust files in this project",
        accept: &[
            "find . -name '*.rs'",
            "find . -type f -name '*.rs'",
            "find . -name \"*.rs\"",
        ],
    },
    Case {
        prompt: "make a new directory called build",
        accept: &["mkdir build", "mkdir ./build"],
    },
    Case {
        prompt: "remove the file old.log",
        accept: &["rm old.log", "rm ./old.log"],
    },
    Case {
        prompt: "show the last 20 lines of server.log",
        accept: &["tail -n 20 server.log", "tail -20 server.log"],
    },
    Case {
        prompt: "show running processes",
        accept: &["ps", "ps aux", "ps -ef"],
    },
];

fn fixture_shell() -> ShellContext {
    // Pinned context so eval results are reproducible across machines.
    ShellContext {
        kind: ShellKind::Bash,
        cwd: None,
        os: "linux",
        history: Vec::new(),
    }
}

fn fixture_config() -> Config {
    let mut cfg = Config::default();
    if let Ok(model) = std::env::var("INTERPRETER_EVAL_MODEL") {
        cfg.model = model;
    }
    cfg.temperature = Some(0.0);
    cfg
}

fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn run_case(case: &Case) {
    let cfg = fixture_config();
    let shell = fixture_shell();
    let got = llm_client::generate_command(&cfg, &shell, case.prompt)
        .await
        .unwrap_or_else(|e| panic!("prompt {:?} failed: {e:#}", case.prompt));
    let got_n = normalize(&got);
    let ok = case.accept.iter().any(|a| normalize(a) == got_n);
    assert!(
        ok,
        "prompt: {:?}\n got: {:?}\n accept: {:?}",
        case.prompt, got, case.accept
    );
}

#[tokio::test]
#[ignore = "hits a live LLM; run with --ignored"]
async fn eval_all_cases() {
    let mut failures = Vec::new();
    for case in CASES {
        let cfg = fixture_config();
        let shell = fixture_shell();
        match llm_client::generate_command(&cfg, &shell, case.prompt).await {
            Ok(got) => {
                let got_n = normalize(&got);
                let ok = case.accept.iter().any(|a| normalize(a) == got_n);
                if !ok {
                    failures.push(format!(
                        "  - {:?}\n      got:    {:?}\n      accept: {:?}",
                        case.prompt, got, case.accept
                    ));
                }
            }
            Err(e) => failures.push(format!("  - {:?} errored: {e:#}", case.prompt)),
        }
    }
    assert!(
        failures.is_empty(),
        "\n{} of {} cases failed:\n{}",
        failures.len(),
        CASES.len(),
        failures.join("\n")
    );
}

// Individual cases so a failure surfaces the specific prompt in test output.
macro_rules! case_test {
    ($name:ident, $idx:expr) => {
        #[tokio::test]
        #[ignore = "hits a live LLM; run with --ignored"]
        async fn $name() {
            run_case(&CASES[$idx]).await;
        }
    };
}

case_test!(case_00_ls, 0);
case_test!(case_01_ls_a, 1);
case_test!(case_02_pwd, 2);
case_test!(case_03_cat_readme, 3);
case_test!(case_04_wc_l, 4);
case_test!(case_05_find_rs, 5);
case_test!(case_06_mkdir, 6);
case_test!(case_07_rm, 7);
case_test!(case_08_tail, 8);
case_test!(case_09_ps, 9);
