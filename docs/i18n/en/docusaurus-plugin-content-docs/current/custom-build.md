---
title: Custom Build
sidebar_position: 6
---

# Custom Build (Cargo Features)

OxiDNS exposes optional protocols, optional plugins, and their external
dependencies as independent Cargo features. Fork the repository and either
change the `default = [...]` entry in `Cargo.toml`, or pass `--features`
on the command line, to produce a binary tailored to your scenario.

> By default `cargo build` enables the `full` bundle and produces a binary
> identical to the published release. You only enter "slimmed" mode when
> you explicitly pass `--no-default-features`.

## Three preset bundles

| Bundle | Use case | Roughly contains |
|---|---|---|
| `minimal` | Embedded / container / experimentation | UDP + TCP listeners, UDP + TCP upstreams, basic executors (sequence / forward / cache / fallback / hosts / redirect / arbitrary / dual_selector / ecs_handler / ttl / drop_resp / black_hole / debug_print / reload), all matchers, `domain_set` + `ip_set` providers |
| `standard` | Home router / mid-range | minimal + DoQ ingress & upstream + `provider-protobuf` (geoip / geosite / v2ray_dat) + adguard_rule + cron + script + download + http_request + reverse_lookup |
| `full` (default) | Everything | standard + DoH3 ingress & upstream + MikroTik integration + query_recorder + ipset / nftset + the `upgrade` subcommand |

## Granular toggles

Each feature below is independently switchable. The bundle features are
just collections of these — you can also pick individual toggles and skip
the presets entirely.

### Protocols

| Feature | Effect |
|---|---|
| `server-doq` | Enable DoQ (QUIC) inbound server, requires `quinn` |
| `server-doh3` | Enable HTTP/3 leg of the DoH server, requires `h3` / `h3-quinn` / `quinn` |
| `upstream-doq` | Enable DoQ upstreams (`quic://` / `doq://` schemes) |
| `upstream-doh3` | Enable HTTP/3 DoH upstreams (`h3://` scheme or `enable_http3: true`) |

> When `upstream-doq` is off, configs that still reference `quic://...`
> fail at startup with `upstream DoQ is not compiled into this build;
> rebuild with --features upstream-doq` instead of crashing.

### Management plane / protocols (Phase 1B)

`api`, `webui`, `metrics`, `server-dot`, `server-doh`, `upstream-dot`, and
`upstream-doh` require lifting `AppController` / `LogBuffer` out of
`src/api/` into `src/core/` first; they will open up in a follow-up
release. For now those capabilities stay **always-on and not yet
strippable**.

### Optional plugins

| Feature | Plugin | Main dependency |
|---|---|---|
| `plugin-mikrotik` | `ros_address_list` | `mikrotik-rs` |
| `plugin-query-recorder` | `query_recorder` | `rusqlite` (bundled SQLite) |
| `plugin-ipset` | `ipset` + `nftset` | `ripset` (Linux only) |
| `plugin-cron` | `cron` | `cronexpr` |
| `plugin-script` | `script` | — |
| `plugin-download` | `download` | — |
| `plugin-http-request` | `http_request` | — |
| `plugin-reverse-lookup` | `reverse_lookup` | — |
| `plugin-upgrade` | `upgrade` CLI subcommand + `upgrade` executor | `flate2` / `tar` / `zip` (Windows) / `semver` |
| `provider-protobuf` | `geoip` + `geosite` + `v2ray_dat` (share `prost`) | `prost` |
| `provider-adguard-rule` | `adguard_rule` | — |

## Common build commands

```bash
# Default full build (== published release)
cargo build --release

# Smallest build: bare forwarder only
cargo build --release --no-default-features --features minimal

# Home-router build (DoQ + common geo/adguard providers + popular executors)
cargo build --release --no-default-features --features standard

# Minimal plus only the MikroTik integration
cargo build --release --no-default-features --features "minimal,plugin-mikrotik"
```

## Verifying the feature matrix

The repo ships `just` recipes that exercise all three bundles plus the
default-features test suite in one go:

```bash
just check-matrix
```

Or run them individually:

```bash
just check-minimal   # cargo +nightly clippy --no-default-features --features minimal
just check-standard
just check-full      # cargo +nightly clippy --all-features
```

## Runtime behavior for missing plugins

When a feature is off, the matching `#[plugin_factory("...")]` registration
block is not compiled, so the plugin type name never enters the global
factory table. A config that references a plugin not compiled into the
binary is rejected at startup by `analyze_configuration`:

```
Error: Plugin("Unknown plugin type: query_recorder")
```

This is the intended behavior — the user sees a clean error instead of a
mid-run crash.

## Common patterns after forking

1. Change `default = ["standard"]` (or any custom combination) in
   `Cargo.toml` so that `cargo build` and `cargo install` both produce the
   tailored binary out of the box.
2. If you want automatic updates against your own fork, override the
   defaults of the `upgrade` subcommand (`--repository`, `--asset`) so
   `oxidns upgrade` looks at your release feed.
