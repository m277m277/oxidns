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

| Bundle | Positioning | Size reference |
|---|---|---|
| `minimal` | Low-memory devices / container / experimentation; bare UDP+TCP forwarder | ~8.9 MB |
| `standard` | Home router; WebUI + encrypted protocols + common plugins | Between the two |
| `full` (default) | Everything — matches the published release | ~21 MB |

> When you fork and compose features yourself, treat `oxidns build-info`
> or `GET /api/build` as the source of truth for the running binary. The
> sections below describe the exact contents of each official preset.

### `minimal` — bare forwarder

Compiles only what the DNS forwarder strictly needs; drops HTTP, the encrypted
protocols, and every optional plugin. Dependencies on hyper / rustls / quinn /
h2 / h3 / sqlite / zoneparser are excluded, making this the smallest bundle.

**Includes**

- Inbound: UDP, TCP
- Upstream: UDP, TCP (cleartext)
- Providers: `domain_set`, `ip_set`
- Matchers: all
- Executors: `sequence`, `forward`, `cache`, `fallback`, `hosts`, `redirect`,
  `dual_selector`, `ecs_handler`, `ttl`, `drop_resp`, `black_hole`,
  `debug_print`, `reload`
- In-process metric counters (no HTTP endpoint)

**Excludes**

- Management API / WebUI / Prometheus `/metrics`
- DoT / DoH / DoQ / DoH3 protocols
- All optional plugins and the `upgrade` subcommand

### `standard` — home router

Adds the management plane, encrypted protocol stacks, and common plugins on
top of `minimal`.

**Adds**

- Management: HTTP API, WebUI, Prometheus `/metrics`, `metrics_collector`
- Inbound / upstream: DoT, DoH (HTTP/2), DoQ
- Providers: `geoip`, `geosite`, `v2ray_dat`, `adguard_rule`
- Executors: `arbitrary`, `cron`, `download`, `http_request`,
  `reverse_lookup`, `query_recorder`, `script`
- `upgrade` CLI subcommand and `upgrade` executor

### `full` (default) — everything

Adds DoH3 and platform integrations on top of `standard`.

**Adds**

- Inbound / upstream: DoH HTTP/3
- Executors: `ros_address_list` (MikroTik), `ipset`, `nftset`

### Official release artifacts

| Channel | minimal | standard | full |
|---|---|---|---|
| Linux x86_64 / ARM64 musl slim archive | ✓ (no WebUI) | ✓ (with WebUI) | — |
| Full release-target matrix | — | — | ✓ |
| `.deb` / Docker | — | — | ✓ |

## Granular toggles

Each feature below is independently switchable. The bundle features are
just collections of these — you can also pick individual toggles and skip
the presets entirely.

### Inbound / outbound protocols

| Feature | Effect |
|---|---|
| `server-dot` | Enable DoT (TLS over TCP) inbound server, requires the rustls server stack |
| `server-doh` | Enable DoH (HTTP/2 over TLS) inbound server, requires hyper server + rustls |
| `server-doq` | Enable DoQ (QUIC) inbound server, requires `quinn` |
| `server-doh3` | Enable the HTTP/3 leg of the DoH server (needs `server-doh`), adds `h3` / `h3-quinn` / `quinn` |
| `upstream-dot` | Enable DoT upstreams (`tls://` scheme), requires the rustls client stack |
| `upstream-doh` | Enable DoH (HTTP/2) upstreams (`https://` scheme), requires hyper-rustls + `h2` |
| `upstream-doq` | Enable DoQ upstreams (`quic://` / `doq://` schemes) |
| `upstream-doh3` | Enable HTTP/3 DoH upstreams (`h3://` scheme or `enable_http3: true`, needs `upstream-doh`) |

> When a protocol is off, configs that still reference its scheme/fields
> fail at startup with a clear message, e.g. `upstream DoT is not compiled
> into this build; rebuild with --features upstream-dot`, instead of
> crashing. With `server-dot` off, putting `cert` / `key` on a
> `tcp_server` yields `DoT is not compiled into this build; rebuild with
> --features server-dot`.

### Management plane

| Feature | Effect | Dependency |
|---|---|---|
| `api` | Management / health / control / logs / config HTTP API, plus each plugin's `/plugins/<tag>/...` endpoints | hyper server + rustls server (for HTTPS) |
| `webui` | Serve the WebUI static assets from the API hub (requires `api`) | — |
| `metrics` | `/metrics` Prometheus endpoint + the `metrics_collector` executor (requires `api`) | — |

> With `api` off, the whole `src/api/` module is dropped and the hyper /
> rustls server stack goes with it — this is the main reason `minimal`
> shrinks so much. The in-process `MetricSource` counters always stay in
> core, so turning off `metrics` only removes the HTTP surface and never
> touches the hot path. `AppController` / `LogBuffer` now live in
> `src/core/`, so the core runtime (reload, shutdown, the log ring buffer)
> still works in a `minimal` build that has no `api`.

### Optional plugins

| Feature | Plugin | Main dependency |
|---|---|---|
| `plugin-mikrotik` | `ros_address_list` | `mikrotik-rs` |
| `plugin-query-recorder` | `query_recorder` | `rusqlite` (bundled SQLite) |
| `plugin-ipset` | `ipset` + `nftset` | `ripset` (Linux only) |
| `plugin-cron` | `cron` | `cronexpr` |
| `plugin-script` | `script` | — |
| `plugin-arbitrary` | `arbitrary` | `oxidns-zoneparser` |
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

# Home-router build (API + DoT/DoH/DoQ + common geo/adguard providers + executors)
cargo build --release --no-default-features --features standard

# Minimal plus only the MikroTik integration
cargo build --release --no-default-features --features "minimal,plugin-mikrotik"

# Bare forwarder plus the management API, nothing heavy
cargo build --release --no-default-features --features "minimal,api"
```

Official release archives remain `full` by default. Linux x86_64 / ARM64 musl
also get `minimal` / `standard` slim archives named like
`oxidns-standard-x86_64-unknown-linux-musl.tar.gz`. The `minimal` archive only
contains the binary, default config, and license; the `standard` archive also
includes WebUI static files, query_recorder, and the `upgrade` subcommand.

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
   defaults of the `upgrade` subcommand (`--repository`, `--asset` /
   `--bundle`) so `oxidns upgrade` looks at your release feed. Custom builds
   should not rely on `bundle: auto`; set `asset` explicitly.
