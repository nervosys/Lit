//! Working out who a request actually came from.
//!
//! Lit identifies a client by the peer address of the TCP connection. That is
//! the right default and the only one that cannot be lied about — but behind a
//! reverse proxy it is always the proxy, which costs two things:
//!
//! - the per-address rate limiter becomes a single shared bucket, so one noisy
//!   client starves everyone;
//! - the audit `source` field records the proxy rather than the client, which
//!   is a material loss for NIST SP 800-171r3 `03.03.02`.
//!
//! `X-Forwarded-For` fixes both and is trivially forgeable, so it is honoured
//! **only** when the connection comes from an address the operator has named as
//! a proxy. With no trusted proxies configured — the default — the header is
//! ignored entirely and the peer address is used, exactly as before.
//!
//! Getting this wrong is worse than not doing it: a server that trusts the
//! header from anyone lets every client choose what its audit records say and
//! which rate-limit bucket it lands in.

use std::net::IpAddr;

/// Most `X-Forwarded-For` entries to examine.
///
/// The header is caller-controlled and unbounded. A real chain is a handful of
/// hops; anything longer is someone padding the header, and walking all of it
/// is work they chose for us.
const MAX_FORWARDED_HOPS: usize = 16;

/// The address to treat as the client's.
///
/// `peer` is the TCP peer. `forwarded` is the raw `X-Forwarded-For` value, if
/// the request carried one. `trusted` is the set of addresses the operator has
/// declared to be proxies.
///
/// The header lists addresses left to right, oldest first: `client, proxy1,
/// proxy2`. Each proxy appends the address it received from, so entries to the
/// right are the ones added most recently and are therefore the most
/// trustworthy. We walk from the right, stepping over addresses that are
/// themselves trusted proxies, and take the first one that is not — that is the
/// earliest address we have a trusted party's word for. Anything further left
/// was supplied by the client and is worthless.
pub fn client_address(
    peer: Option<IpAddr>,
    forwarded: Option<&str>,
    trusted: &[IpAddr],
) -> Option<IpAddr> {
    let peer = peer?;

    // No configured proxies, or this connection is not from one: the header is
    // not evidence of anything.
    if trusted.is_empty() || !trusted.contains(&peer) {
        return Some(peer);
    }

    let Some(forwarded) = forwarded else {
        return Some(peer);
    };

    let hops: Vec<&str> = forwarded.split(',').collect();
    let examined = hops.len().min(MAX_FORWARDED_HOPS);

    for entry in hops[hops.len() - examined..].iter().rev() {
        let Some(addr) = parse_forwarded_entry(entry) else {
            // A malformed entry means the chain cannot be reasoned about past
            // this point. Stop rather than skip it: skipping would let a client
            // hide a hop behind a deliberately broken one.
            break;
        };
        if !trusted.contains(&addr) {
            return Some(addr);
        }
    }

    // Every entry we looked at was a trusted proxy, or the header was unusable.
    Some(peer)
}

/// Parse one `X-Forwarded-For` entry.
///
/// Entries are bare addresses in the common case, but an IPv6 address may be
/// bracketed and any entry may carry a port, so both are tolerated.
fn parse_forwarded_entry(entry: &str) -> Option<IpAddr> {
    let entry = entry.trim();
    if entry.is_empty() {
        return None;
    }

    // `[::1]:8080` or `[::1]`
    if let Some(rest) = entry.strip_prefix('[') {
        let (inside, _) = rest.split_once(']')?;
        return inside.parse().ok();
    }

    if let Ok(addr) = entry.parse::<IpAddr>() {
        return Some(addr);
    }

    // `192.0.2.1:8080`. Only valid for IPv4 — a bare IPv6 address contains
    // colons of its own, and one that parsed would have been caught above.
    let (host, _) = entry.rsplit_once(':')?;
    host.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn with_no_trusted_proxies_the_header_is_ignored() {
        let peer = Some(ip("203.0.113.9"));
        assert_eq!(
            client_address(peer, Some("198.51.100.1"), &[]),
            Some(ip("203.0.113.9")),
            "an unconfigured server must not let a caller pick its own address"
        );
    }

    #[test]
    fn a_header_from_an_untrusted_peer_is_ignored() {
        let peer = Some(ip("203.0.113.9"));
        let trusted = [ip("127.0.0.1")];
        assert_eq!(
            client_address(peer, Some("198.51.100.1"), &trusted),
            Some(ip("203.0.113.9"))
        );
    }

    #[test]
    fn a_header_from_a_trusted_proxy_is_honoured() {
        let peer = Some(ip("127.0.0.1"));
        let trusted = [ip("127.0.0.1")];
        assert_eq!(
            client_address(peer, Some("198.51.100.1"), &trusted),
            Some(ip("198.51.100.1"))
        );
    }

    #[test]
    fn the_rightmost_untrusted_hop_wins() {
        // client, then two proxies that are both ours.
        let peer = Some(ip("10.0.0.1"));
        let trusted = [ip("10.0.0.1"), ip("10.0.0.2")];
        assert_eq!(
            client_address(peer, Some("198.51.100.1, 10.0.0.2, 10.0.0.1"), &trusted),
            Some(ip("198.51.100.1"))
        );
    }

    #[test]
    fn a_client_cannot_hide_behind_addresses_it_prepends() {
        // The client claims a chain of its own before the real one. Everything
        // left of the first untrusted address is its invention, and walking
        // from the right ignores all of it.
        let peer = Some(ip("10.0.0.1"));
        let trusted = [ip("10.0.0.1")];
        assert_eq!(
            client_address(
                peer,
                Some("1.1.1.1, 2.2.2.2, 198.51.100.1, 10.0.0.1"),
                &trusted
            ),
            Some(ip("198.51.100.1")),
            "only the hop the trusted proxy vouched for may be used"
        );
    }

    #[test]
    fn a_malformed_entry_stops_the_walk_rather_than_being_skipped() {
        // Skipping it would let a client hide its real hop behind junk.
        let peer = Some(ip("10.0.0.1"));
        let trusted = [ip("10.0.0.1")];
        assert_eq!(
            client_address(
                peer,
                Some("198.51.100.1, not-an-address, 10.0.0.1"),
                &trusted
            ),
            Some(ip("10.0.0.1")),
            "an unreadable chain falls back to the peer"
        );
    }

    #[test]
    fn a_chain_of_only_trusted_proxies_falls_back_to_the_peer() {
        let peer = Some(ip("10.0.0.1"));
        let trusted = [ip("10.0.0.1"), ip("10.0.0.2")];
        assert_eq!(
            client_address(peer, Some("10.0.0.2, 10.0.0.1"), &trusted),
            Some(ip("10.0.0.1"))
        );
    }

    #[test]
    fn an_absent_or_empty_header_falls_back_to_the_peer() {
        let peer = Some(ip("127.0.0.1"));
        let trusted = [ip("127.0.0.1")];
        assert_eq!(client_address(peer, None, &trusted), Some(ip("127.0.0.1")));
        assert_eq!(
            client_address(peer, Some(""), &trusted),
            Some(ip("127.0.0.1"))
        );
        assert_eq!(
            client_address(peer, Some("   "), &trusted),
            Some(ip("127.0.0.1"))
        );
    }

    #[test]
    fn entries_may_carry_a_port_or_brackets() {
        let peer = Some(ip("127.0.0.1"));
        let trusted = [ip("127.0.0.1")];
        assert_eq!(
            client_address(peer, Some("198.51.100.1:51234"), &trusted),
            Some(ip("198.51.100.1"))
        );
        assert_eq!(
            client_address(peer, Some("[2001:db8::1]:443"), &trusted),
            Some(ip("2001:db8::1"))
        );
        assert_eq!(
            client_address(peer, Some("2001:db8::1"), &trusted),
            Some(ip("2001:db8::1"))
        );
    }

    #[test]
    fn a_padded_header_is_not_walked_indefinitely() {
        // Only the rightmost MAX_FORWARDED_HOPS are examined. Here every one of
        // those is the trusted proxy, so the result falls back to the peer
        // rather than reaching the client's padding far to the left.
        let peer = Some(ip("10.0.0.1"));
        let trusted = [ip("10.0.0.1")];
        let mut chain = vec!["198.51.100.1"];
        for _ in 0..MAX_FORWARDED_HOPS {
            chain.push("10.0.0.1");
        }
        assert_eq!(
            client_address(peer, Some(&chain.join(", ")), &trusted),
            Some(ip("10.0.0.1"))
        );
    }

    #[test]
    fn a_request_with_no_peer_address_has_no_client_address() {
        assert_eq!(
            client_address(None, Some("198.51.100.1"), &[ip("127.0.0.1")]),
            None
        );
    }
}
