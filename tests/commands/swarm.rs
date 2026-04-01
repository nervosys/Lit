/// Command tests for `lit swarm`
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid test interference.
use tempfile::TempDir;

fn init_test_repo() -> TempDir {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();
    lit::commands::init::execute(false, Some(repo_path)).unwrap();
    temp
}

#[test]
fn test_swarm_register_agent() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::swarm::execute_register("agent-001".to_string());
    assert!(
        result.is_ok(),
        "Register agent should succeed: {:?}",
        result.err()
    );
}

#[test]
fn test_swarm_list_agents() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::swarm::execute_register("agent-a".to_string()).unwrap();
    lit::commands::swarm::execute_register("agent-b".to_string()).unwrap();

    let result = lit::commands::swarm::execute_list();
    assert!(result.is_ok(), "List agents should succeed");
}

#[test]
fn test_swarm_lease_acquire_and_release() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::swarm::execute_register("agent-x".to_string()).unwrap();

    let result = lit::commands::swarm::execute_lease_acquire(
        "agent-x".to_string(),
        "src/main.rs".to_string(),
        60,
    );
    assert!(
        result.is_ok(),
        "Lease acquire should succeed: {:?}",
        result.err()
    );

    let result = lit::commands::swarm::execute_lease_release(
        "agent-x".to_string(),
        "src/main.rs".to_string(),
    );
    assert!(
        result.is_ok(),
        "Lease release should succeed: {:?}",
        result.err()
    );
}

#[test]
fn test_swarm_lease_conflict() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::swarm::execute_register("agent-1".to_string()).unwrap();
    lit::commands::swarm::execute_register("agent-2".to_string()).unwrap();

    lit::commands::swarm::execute_lease_acquire(
        "agent-1".to_string(),
        "shared.txt".to_string(),
        3600,
    )
    .unwrap();

    let result = lit::commands::swarm::execute_lease_acquire(
        "agent-2".to_string(),
        "shared.txt".to_string(),
        3600,
    );
    assert!(
        result.is_err(),
        "Second agent should fail to acquire held lease"
    );
}

#[test]
fn test_swarm_lease_list() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::swarm::execute_register("agent-z".to_string()).unwrap();
    lit::commands::swarm::execute_lease_acquire("agent-z".to_string(), "file1.rs".to_string(), 60)
        .unwrap();

    let result = lit::commands::swarm::execute_lease_list();
    assert!(result.is_ok(), "Lease list should succeed");
}
