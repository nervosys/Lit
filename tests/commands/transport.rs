/// Integration tests for transport detection

#[test]
fn test_https_url_rejected_by_resolve() {
    let result = lit::network::transport::resolve_url("https://github.com/example/repo.git");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("cannot be resolved to a local path"),
        "Error should explain HTTPS needs RemoteRepo::open(): {}",
        err
    );
}

#[test]
fn test_ssh_url_rejected_by_resolve() {
    let result = lit::network::transport::resolve_url("ssh://git@github.com/example/repo.git");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("cannot be resolved to a local path"),
        "Error should explain SSH needs RemoteRepo::open(): {}",
        err
    );
}

#[test]
fn test_lit_protocol_url_rejected_by_resolve() {
    let result = lit::network::transport::resolve_url("lit://example.com/repo");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("cannot be resolved to a local path"),
        "Error should explain lit:// needs RemoteRepo::open(): {}",
        err
    );
}

#[test]
fn test_transport_detection() {
    use lit::network::transport::{detect_transport, TransportKind};

    assert_eq!(
        detect_transport("https://github.com/repo"),
        TransportKind::Https
    );
    assert_eq!(
        detect_transport("http://example.com/repo"),
        TransportKind::Https
    );
    assert_eq!(detect_transport("ssh://git@host/repo"), TransportKind::Ssh);
    assert_eq!(
        detect_transport("lit://example.com/repo"),
        TransportKind::Lit
    );
    assert_eq!(
        detect_transport("/local/path/to/repo"),
        TransportKind::Local
    );
    assert_eq!(
        detect_transport("file:///path/to/repo"),
        TransportKind::Local
    );
    assert_eq!(detect_transport("../relative/path"), TransportKind::Local);
}
