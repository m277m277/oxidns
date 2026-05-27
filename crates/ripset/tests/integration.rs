//! End-to-end integration tests against a live Linux kernel.
//!
//! All tests are gated on `target_os = "linux"` and marked `#[ignore]` so
//! `cargo test` stays runnable on macOS / Windows and in CI environments
//! without elevated privileges. To exercise the real netlink path:
//!
//! ```sh
//! sudo cargo test --package oxidns-ripset --test integration -- --ignored --test-threads=1
//! ```
//!
//! Each test sets up its own uniquely-named table/set, asserts kernel
//! state via the `nft` / `ipset` userspace binaries, and tears the
//! resources down even on assertion failure. Coverage targets OxiDNS's
//! production usage of `nftset_add` / `ipset_add` plus the surrounding
//! create / test / delete / list APIs.

#![cfg(target_os = "linux")]

mod harness;

use std::net::IpAddr;

use harness::{NftCleanup, ensure_nft_available, run_nft, unique_name};
use ripset::{
    IpCidr, IpEntry, IpSetError, NftSetCreateOptions, NftSetType, nftset_add, nftset_create_set,
    nftset_create_table, nftset_del, nftset_delete_set, nftset_delete_table, nftset_list,
    nftset_list_tables, nftset_test,
};

/// Smoke test for OxiDNS's primary nftset use case: add /32 entries to a
/// `flags interval` ipv4 set under the `ip` family. Mirrors the exact
/// shape of the proxy_set configuration from issues #122 / #127.
#[test]
#[ignore]
fn nftset_add_slash32_to_interval_ipv4_set() {
    ensure_nft_available();
    let table = unique_name("oxi_nft_ipv4");
    let set = "v4_set";
    let _g = NftCleanup::new("ip", &table);

    run_nft(&["add", "table", "ip", &table]);
    run_nft(&[
        "add",
        "set",
        "ip",
        &table,
        set,
        "{ type ipv4_addr; flags interval; }",
    ]);

    let cidr = IpCidr::new("185.45.5.35".parse().unwrap(), 32).unwrap();
    nftset_add("ip", &table, set, cidr).expect("add /32 must succeed on interval set");

    let listing = run_nft(&["list", "set", "ip", &table, set]);
    assert!(
        listing.contains("185.45.5.35"),
        "set listing should contain the added IP, got:\n{listing}"
    );
}

/// Repeating the same /32 add must not cascade into a fatal error.
/// OxiDNS retries the same answer on every DNS query for popular
/// domains. The library issues NEWSETELEM with NLM_F_CREATE (no
/// NLM_F_EXCL), so the kernel's documented behaviour is either
/// "silently succeed" or "return EEXIST" depending on version. Either
/// outcome is fine — what matters is that we never tear the plugin
/// down on the second add (issue #122).
#[test]
#[ignore]
fn nftset_duplicate_add_is_idempotent_or_element_exists() {
    ensure_nft_available();
    let table = unique_name("oxi_nft_dup");
    let _g = NftCleanup::new("ip", &table);

    run_nft(&["add", "table", "ip", &table]);
    run_nft(&[
        "add",
        "set",
        "ip",
        &table,
        "s",
        "{ type ipv4_addr; flags interval; }",
    ]);

    let cidr = IpCidr::new("1.2.3.4".parse().unwrap(), 32).unwrap();
    nftset_add("ip", &table, "s", cidr).unwrap();
    match nftset_add("ip", &table, "s", cidr) {
        Ok(()) | Err(IpSetError::ElementExists) => {}
        other => panic!("expected Ok or ElementExists on duplicate add, got {other:?}"),
    }
    // Regardless of how the second add was reported, the IP must still
    // be in the set after both calls.
    let listing = run_nft(&["list", "set", "ip", &table, "s"]);
    assert!(listing.contains("1.2.3.4"));
}

/// IPv6 path: /128 add into a `flags interval` ipv6_addr set under
/// family `ip6`. Validates the v6 keying (16-byte big-endian) end to
/// end.
#[test]
#[ignore]
fn nftset_add_slash128_to_interval_ipv6_set() {
    ensure_nft_available();
    let table = unique_name("oxi_nft_ipv6");
    let _g = NftCleanup::new("ip6", &table);

    run_nft(&["add", "table", "ip6", &table]);
    run_nft(&[
        "add",
        "set",
        "ip6",
        &table,
        "s",
        "{ type ipv6_addr; flags interval; }",
    ]);

    let cidr = IpCidr::new("2001:db8::1".parse().unwrap(), 128).unwrap();
    nftset_add("ip6", &table, "s", cidr).unwrap();

    let listing = run_nft(&["list", "set", "ip6", &table, "s"]);
    assert!(
        listing.contains("2001:db8::1"),
        "expected 2001:db8::1 in:\n{listing}"
    );
}

/// `inet` family must work the same as `ip`. OxiDNS users sometimes
/// share one inet table between v4 and v6 sets.
#[test]
#[ignore]
fn nftset_add_in_inet_family() {
    ensure_nft_available();
    let table = unique_name("oxi_nft_inet");
    let _g = NftCleanup::new("inet", &table);

    run_nft(&["add", "table", "inet", &table]);
    run_nft(&[
        "add",
        "set",
        "inet",
        &table,
        "s",
        "{ type ipv4_addr; flags interval; }",
    ]);

    let cidr = IpCidr::new("8.8.8.8".parse().unwrap(), 32).unwrap();
    nftset_add("inet", &table, "s", cidr).unwrap();

    let listing = run_nft(&["list", "set", "inet", &table, "s"]);
    assert!(listing.contains("8.8.8.8"));
}

/// A CIDR add into a non-interval set must be refused at the library
/// layer (`UnsupportedEntry`) rather than producing a netlink error
/// surfaced to callers — this keeps the OxiDNS executor's "skip
/// gracefully" path predictable.
#[test]
#[ignore]
fn nftset_cidr_add_to_non_interval_set_is_unsupported_entry() {
    ensure_nft_available();
    let table = unique_name("oxi_nft_noiv");
    let _g = NftCleanup::new("ip", &table);

    run_nft(&["add", "table", "ip", &table]);
    // No `flags interval` → kernel won't accept ranges.
    run_nft(&["add", "set", "ip", &table, "s", "{ type ipv4_addr; }"]);

    let cidr = IpCidr::new("1.2.3.4".parse().unwrap(), 32).unwrap();
    let err = nftset_add("ip", &table, "s", cidr).unwrap_err();
    assert!(
        matches!(err, IpSetError::UnsupportedEntry(_)),
        "expected UnsupportedEntry, got {err:?}"
    );
}

/// /24 add must store the whole subnet as a single interval. Verifies
/// the end-key (exclusive) math (start=1.2.3.0, end=1.2.4.0) and the
/// `nft list` shows the prefix form.
#[test]
#[ignore]
fn nftset_add_slash24_subnet() {
    ensure_nft_available();
    let table = unique_name("oxi_nft_s24");
    let _g = NftCleanup::new("ip", &table);

    run_nft(&["add", "table", "ip", &table]);
    run_nft(&[
        "add",
        "set",
        "ip",
        &table,
        "s",
        "{ type ipv4_addr; flags interval; }",
    ]);

    let cidr = IpCidr::new("1.2.3.0".parse().unwrap(), 24).unwrap();
    nftset_add("ip", &table, "s", cidr).unwrap();

    let listing = run_nft(&["list", "set", "ip", &table, "s"]);
    assert!(
        listing.contains("1.2.3.0/24"),
        "expected 1.2.3.0/24 in listing:\n{listing}"
    );
}

/// Full lifecycle: add → test (true) → del → test (false). Exercises
/// every operation OxiDNS could plausibly want from the crate.
#[test]
#[ignore]
fn nftset_add_test_del_test_lifecycle() {
    ensure_nft_available();
    let table = unique_name("oxi_nft_life");
    let _g = NftCleanup::new("ip", &table);

    run_nft(&["add", "table", "ip", &table]);
    run_nft(&[
        "add",
        "set",
        "ip",
        &table,
        "s",
        "{ type ipv4_addr; flags interval; }",
    ]);

    let cidr = IpCidr::new("10.20.30.40".parse().unwrap(), 32).unwrap();
    nftset_add("ip", &table, "s", cidr).unwrap();
    assert!(nftset_test("ip", &table, "s", cidr).unwrap());

    nftset_del("ip", &table, "s", cidr).unwrap();
    assert!(!nftset_test("ip", &table, "s", cidr).unwrap());
}

/// Bulk-add path: OxiDNS can produce hundreds of new IPs in a single
/// DNS reply batch (CDN domains). Sustained throughput must not leak
/// state or accumulate errors.
#[test]
#[ignore]
fn nftset_bulk_add_500_unique_ips() {
    ensure_nft_available();
    let table = unique_name("oxi_nft_bulk");
    let _g = NftCleanup::new("ip", &table);

    run_nft(&["add", "table", "ip", &table]);
    run_nft(&[
        "add",
        "set",
        "ip",
        &table,
        "s",
        "{ type ipv4_addr; flags interval; }",
    ]);

    for i in 0..500u32 {
        let addr = IpAddr::V4((std::net::Ipv4Addr::from(0x0A_00_00_00u32 + i)).into());
        let cidr = IpCidr::new(addr, 32).unwrap();
        nftset_add("ip", &table, "s", cidr).expect("bulk add must not fail");
    }

    let listing = run_nft(&["list", "set", "ip", &table, "s"]);
    // Spot-check the first and last entries.
    assert!(listing.contains("10.0.0.0"));
    assert!(listing.contains("10.0.1.243"));
}

/// `nftset_with_timeout`: the entry must enter the kernel set with a
/// timeout visible in `nft list`. Requires the set to be created with
/// `timeout <default>` so per-element timeouts are accepted.
#[test]
#[ignore]
fn nftset_add_with_per_element_timeout() {
    ensure_nft_available();
    let table = unique_name("oxi_nft_to");
    let _g = NftCleanup::new("ip", &table);

    run_nft(&["add", "table", "ip", &table]);
    run_nft(&[
        "add",
        "set",
        "ip",
        &table,
        "s",
        "{ type ipv4_addr; flags interval, timeout; timeout 1h; }",
    ]);

    let addr: IpAddr = "9.9.9.9".parse().unwrap();
    let entry = IpEntry::with_timeout(addr, 60);
    nftset_add("ip", &table, "s", entry).unwrap();

    let listing = run_nft(&["list", "set", "ip", &table, "s"]);
    assert!(listing.contains("9.9.9.9"), "missing IP in {listing}");
    assert!(
        listing.contains("expires "),
        "per-element timeout should be visible in `nft list`:\n{listing}"
    );
}

/// `nftset_create_set` + `nftset_create_table` + `nftset_delete_set` +
/// `nftset_delete_table` create / tear down through the library
/// itself, without leaning on the `nft` cli.
#[test]
#[ignore]
fn nftset_create_and_delete_via_library() {
    ensure_nft_available();
    let table = unique_name("oxi_nft_lib");
    let _g = NftCleanup::new("ip", &table);

    nftset_create_table("ip", &table).expect("create table");
    let opts = NftSetCreateOptions {
        set_type: NftSetType::Ipv4Net,
        timeout: None,
        flags: None,
    };
    nftset_create_set("ip", &table, "via_lib", &opts).expect("create set");

    // List should now show the new set.
    let tables = nftset_list_tables("ip").expect("list tables");
    assert!(
        tables.iter().any(|t| t == &table),
        "expected '{table}' in {tables:?}"
    );

    // Add and read back.
    let cidr = IpCidr::new("172.16.0.0".parse().unwrap(), 24).unwrap();
    nftset_add("ip", &table, "via_lib", cidr).unwrap();
    let entries = nftset_list("ip", &table, "via_lib").expect("list entries");
    assert!(
        !entries.is_empty(),
        "nftset_list should return the entry we just added"
    );

    // Delete.
    nftset_delete_set("ip", &table, "via_lib").expect("delete set");
    nftset_delete_table("ip", &table).expect("delete table");
    // Cleanup guard now becomes a no-op (table already gone).
}

// -----------------------------------------------------------------------
// ipset: hash:ip flows used by the OxiDNS `ipset` executor.
// -----------------------------------------------------------------------

use harness::{IpsetCleanup, ensure_ipset_available, run_ipset};
use ripset::{ipset_add, ipset_create, ipset_del, ipset_list, ipset_test};

/// Primary OxiDNS ipset path: `hash:ip` set populated from DNS
/// answers. Add one v4 IP, then verify it via the `ipset` cli.
#[test]
#[ignore]
fn ipset_add_ipv4_to_hash_ip_set() {
    ensure_ipset_available();
    let name = unique_name("oxi_ips_v4");
    let _g = IpsetCleanup::new(&name);

    run_ipset(&["create", &name, "hash:ip"]);
    let addr: IpAddr = "172.17.0.1".parse().unwrap();
    ipset_add(&name, addr).unwrap();

    let listing = run_ipset(&["list", &name]);
    assert!(listing.contains("172.17.0.1"));
}

/// IPv6 equivalent of the v4 add path.
#[test]
#[ignore]
fn ipset_add_ipv6_to_hash_ip_set() {
    ensure_ipset_available();
    let name = unique_name("oxi_ips_v6");
    let _g = IpsetCleanup::new(&name);

    run_ipset(&["create", &name, "hash:ip", "family", "inet6"]);
    let addr: IpAddr = "fd00::1".parse().unwrap();
    ipset_add(&name, addr).unwrap();

    let listing = run_ipset(&["list", &name]);
    assert!(listing.contains("fd00::1"));
}

/// Full lifecycle on hash:ip: add → test → del → test.
#[test]
#[ignore]
fn ipset_add_test_del_test_lifecycle() {
    ensure_ipset_available();
    let name = unique_name("oxi_ips_life");
    let _g = IpsetCleanup::new(&name);

    run_ipset(&["create", &name, "hash:ip"]);
    let addr: IpAddr = "10.99.0.1".parse().unwrap();

    ipset_add(&name, addr).unwrap();
    assert!(ipset_test(&name, addr).unwrap());

    ipset_del(&name, addr).unwrap();
    assert!(!ipset_test(&name, addr).unwrap());
}

/// hash:net for CIDR adds (OxiDNS uses /24 etc. when configured).
#[test]
#[ignore]
fn ipset_add_cidr_to_hash_net_set() {
    ensure_ipset_available();
    let name = unique_name("oxi_ips_net");
    let _g = IpsetCleanup::new(&name);

    run_ipset(&["create", &name, "hash:net"]);
    let cidr = IpCidr::new("192.168.10.0".parse().unwrap(), 24).unwrap();
    ipset_add(&name, cidr).unwrap();

    let listing = run_ipset(&["list", &name]);
    assert!(listing.contains("192.168.10.0/24"));
}

/// Duplicate add must not be fatal. ipset's classic kernel semantics
/// return EEXIST on duplicate add, but the request goes out without
/// NLM_F_EXCL so newer kernels may also silently succeed. Either
/// outcome lets OxiDNS keep running — that's what the test pins.
#[test]
#[ignore]
fn ipset_duplicate_add_is_idempotent_or_element_exists() {
    ensure_ipset_available();
    let name = unique_name("oxi_ips_dup");
    let _g = IpsetCleanup::new(&name);

    run_ipset(&["create", &name, "hash:ip"]);
    let addr: IpAddr = "1.1.1.1".parse().unwrap();
    ipset_add(&name, addr).unwrap();
    match ipset_add(&name, addr) {
        Ok(()) | Err(IpSetError::ElementExists) => {}
        other => panic!("expected Ok or ElementExists on duplicate add, got {other:?}"),
    }
    let listing = run_ipset(&["list", &name]);
    assert!(listing.contains("1.1.1.1"));
}

/// `ipset_create` via the library — covers the symmetry path where
/// callers don't pre-create sets out of band.
#[test]
#[ignore]
fn ipset_create_via_library() {
    use ripset::{IpSetCreateOptions, IpSetFamily, IpSetType};
    ensure_ipset_available();
    let name = unique_name("oxi_ips_libc");
    let _g = IpsetCleanup::new(&name);

    let opts = IpSetCreateOptions {
        set_type: IpSetType::HashIp,
        family: IpSetFamily::Inet,
        timeout: None,
        hashsize: None,
        maxelem: None,
    };
    ipset_create(&name, &opts).expect("create");
    let addr: IpAddr = "203.0.113.7".parse().unwrap();
    ipset_add(&name, addr).unwrap();

    let entries = ipset_list(&name).expect("list");
    assert!(!entries.is_empty());
}
