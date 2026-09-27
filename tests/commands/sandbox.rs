//! Integration tests for `lit sandbox`.
//!
//! These exist for one reason: until they did, nothing called the run path.
//! `hv2-sandbox`'s Linux backend — user and mount namespaces, `pivot_root`,
//! cgroup v2 — was compiled by CI and executed by nothing, so the platform
//! where the real confinement lives was the platform with no coverage. A test
//! that only compiles a sandbox is indistinguishable from the sandbox this
//! command used to be.
//!
//! They go through `execute_run_at` rather than the working directory, so they
//! do not mutate process-global state and can run in parallel.

use lit::commands::sandbox::{execute_init_from, execute_run_at, RunOptions};
use tempfile::TempDir;

/// A repository with one sandbox in it, ready to run something.
fn repo_with_sandbox(name: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_path_buf();
    lit::commands::init::execute(false, Some(root.to_string_lossy().into_owned())).unwrap();
    execute_init_from(root.clone(), root.clone(), Some(name.to_string()))
        .expect("sandbox should initialise");
    (dir, root)
}

/// A command that echoes a marker, spelled for the host we are on.
fn echo(marker: &str) -> Vec<String> {
    if cfg!(windows) {
        vec!["cmd".into(), "/c".into(), format!("echo {}", marker)]
    } else {
        vec!["/bin/sh".into(), "-c".into(), format!("echo {}", marker)]
    }
}

#[test]
fn a_command_runs_confined_and_the_response_says_how_much() {
    let (_dir, root) = repo_with_sandbox("demo");
    let marker = "lit-sandbox-marker-4f2a";

    let out = execute_run_at(root, "demo".into(), echo(marker), RunOptions::default())
        .expect("a best-effort run must not be refused");

    assert_eq!(out.exit_code, Some(0), "message was: {}", out.message);
    assert!(
        out.output.as_deref().unwrap_or_default().contains(marker),
        "the command's stdout should reach the caller, got: {:?}",
        out.output
    );

    // The point of the integration. A run that enforced nothing and said so
    // would be honest but useless; a run that enforced nothing and stayed quiet
    // is the bug this feature had. Every supported platform enforces something.
    assert!(
        !out.controls_enforced.is_empty(),
        "no controls were enforced on this host, which no supported backend should report: \
         enforced={:?} unenforced={:?}",
        out.controls_enforced,
        out.controls_unenforced
    );

    // Any gap must carry its reason. An entry that is a bare control name means
    // `Controls::reason` returned nothing and the operator is told what is
    // missing but not why — which is the failure mode this reporting exists to
    // avoid.
    for gap in &out.controls_unenforced {
        assert!(
            gap.contains(':'),
            "an unenforced control must name its reason, got: {gap:?}"
        );
    }
}

#[test]
fn the_environment_is_scrubbed_not_inherited() {
    // The env scrub predates the confinement work and is the part that stops a
    // build reaching credential helpers and dotfiles. It is worth a test of its
    // own, because it is the guarantee that holds identically on every platform.
    let (_dir, root) = repo_with_sandbox("scrub");
    std::env::set_var("LIT_SANDBOX_LEAK_CANARY", "leaked");

    let cmd = if cfg!(windows) {
        vec![
            "cmd".into(),
            "/c".into(),
            "echo [%LIT_SANDBOX_LEAK_CANARY%]".into(),
        ]
    } else {
        vec![
            "/bin/sh".into(),
            "-c".into(),
            "echo [$LIT_SANDBOX_LEAK_CANARY]".into(),
        ]
    };

    let out = execute_run_at(root, "scrub".into(), cmd, RunOptions::default()).expect("should run");
    let stdout = out.output.unwrap_or_default();
    std::env::remove_var("LIT_SANDBOX_LEAK_CANARY");

    assert!(
        !stdout.contains("leaked"),
        "the caller's environment must not reach the sandbox, got: {stdout:?}"
    );
}

#[test]
fn a_strict_run_either_confines_fully_or_refuses() {
    // Deliberately not asserting one outcome. Whether strict succeeds is a
    // property of the host: Linux with a writable cgroup v2 hierarchy can
    // satisfy the spec, Windows job objects cannot isolate the network and so
    // must refuse. Asserting either specifically would encode one CI runner's
    // configuration as a requirement. What must hold everywhere is that it
    // never *quietly* under-confines — so a success has to report no gaps.
    let (_dir, root) = repo_with_sandbox("strict");
    let options = RunOptions {
        strict: true,
        allow_network: false,
    };

    match execute_run_at(root, "strict".into(), echo("strict-ok"), options) {
        Ok(out) => assert!(
            out.controls_unenforced.is_empty(),
            "a strict run that succeeded must have enforced everything it asked for, \
             yet reported gaps: {:?}",
            out.controls_unenforced
        ),
        Err(e) => assert!(
            e.internal_message().contains("requested confinement"),
            "a strict refusal should say it was about confinement, got: {}",
            e.internal_message()
        ),
    }
}

#[test]
fn a_run_against_a_missing_sandbox_is_refused_not_created() {
    let (_dir, root) = repo_with_sandbox("exists");
    let err = execute_run_at(
        root,
        "does-not-exist".into(),
        echo("nope"),
        RunOptions::default(),
    )
    .expect_err("running a sandbox that was never initialised must fail");
    assert!(
        err.internal_message().contains("not found"),
        "got: {}",
        err.internal_message()
    );
}
