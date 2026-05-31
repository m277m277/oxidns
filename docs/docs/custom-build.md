---
title: 自定义编译
sidebar_position: 6
---

# 自定义编译(Cargo Features)

OxiDNS 通过 Cargo features 把可选协议、可选插件、外部依赖拆成独立开关。
fork 仓库后修改 `Cargo.toml` 的 `default = [...]`,或在编译时用 `--features`
指定,即可裁剪出适合自己场景的精简二进制。

> 默认情况下 `cargo build` 会启用 `full` 组合包,产出和发布版本完全等价
> 的二进制。只有显式 `--no-default-features` 才会进入裁剪模式。

## 三种预设组合

| Bundle | 适用场景 | 大致内容 |
|---|---|---|
| `minimal` | 嵌入式 / 容器 / 学习 | UDP + TCP 监听,UDP + TCP upstream,sequence / forward / cache / fallback / hosts / redirect / arbitrary / dual_selector / ecs_handler / ttl / drop_resp / black_hole / debug_print / reload 等基础执行器,全部 matcher,`domain_set` + `ip_set` provider |
| `standard` | 家用路由器 / 中等规模 | minimal + DoQ 上下行 + provider-protobuf(geoip/geosite/v2ray_dat) + adguard_rule + cron + script + download + http_request + reverse_lookup |
| `full`(默认) | 全功能 | standard + DoH3 上下行 + MikroTik 集成 + query_recorder + ipset / nftset + upgrade 子命令 |

## 颗粒度开关

下表里的每个 feature 都可以单独打开或关闭。组合包就是这些开关的集合,
你也可以**只挑自己需要的开关**而不走预设。

### 协议

| Feature | 作用 |
|---|---|
| `server-doq` | 启用 DoQ(QUIC)入站服务器,依赖 `quinn` |
| `server-doh3` | 在 DoH 服务器上启用 HTTP/3 路径,依赖 `h3` / `h3-quinn` / `quinn` |
| `upstream-doq` | 启用 DoQ upstream(`quic://` / `doq://` scheme) |
| `upstream-doh3` | 启用 DoH HTTP/3 upstream(`h3://` scheme 或 `enable_http3: true`) |

> 关闭 upstream-doq 后,如果 yaml 里仍写 `quic://...`,启动时会以
> "upstream DoQ is not compiled into this build; rebuild with --features
> upstream-doq" 报错,而不是崩溃。

### 管理面 / 协议(Phase 1B)

`api`、`webui`、`metrics`、`server-dot`、`server-doh`、`upstream-dot`、
`upstream-doh` 这几项需要先把 `AppController` / `LogBuffer` 从 `src/api/`
抽到 `src/core/`,会在后续版本一并开放。本阶段它们对应的功能保持
**默认启用且不可裁剪**。

### 可选插件

| Feature | 插件 | 主要依赖 |
|---|---|---|
| `plugin-mikrotik` | `ros_address_list` | `mikrotik-rs` |
| `plugin-query-recorder` | `query_recorder` | `rusqlite`(bundled SQLite) |
| `plugin-ipset` | `ipset` + `nftset` | `ripset`(Linux only) |
| `plugin-cron` | `cron` | `cronexpr` |
| `plugin-script` | `script` | — |
| `plugin-download` | `download` | — |
| `plugin-http-request` | `http_request` | — |
| `plugin-reverse-lookup` | `reverse_lookup` | — |
| `plugin-upgrade` | `upgrade` CLI 子命令 + `upgrade` 执行器 | `flate2` / `tar` / `zip`(Windows) / `semver` |
| `provider-protobuf` | `geoip` + `geosite` + `v2ray_dat`(共享 `prost`) | `prost` |
| `provider-adguard-rule` | `adguard_rule` | — |

## 常用编译命令

```bash
# 默认全功能(等价于发布版本)
cargo build --release

# 最小可用,只跑基础转发
cargo build --release --no-default-features --features minimal

# 家用路由器(含 DoQ、geo、adguard 等常用 provider/executor)
cargo build --release --no-default-features --features standard

# 只在 minimal 上加 MikroTik 集成
cargo build --release --no-default-features --features "minimal,plugin-mikrotik"
```

## 验证编译矩阵

仓库自带 `just` 配方,一次跑完三种组合的 clippy + 默认 feature 的 test:

```bash
just check-matrix
```

或者分别:

```bash
just check-minimal   # cargo +nightly clippy --no-default-features --features minimal
just check-standard
just check-full      # cargo +nightly clippy --all-features
```

## 缺失插件的运行时行为

每个 feature 关闭后,对应插件的 `#[plugin_factory("...")]` 注册块不会
被编译,因此插件类型名也不会出现在全局工厂表里。如果 yaml 配置里使用
了未编译的插件,启动时会被 `analyze_configuration` 拦截:

```
Error: Plugin("Unknown plugin type: query_recorder")
```

这是预期行为 — 用户得到清晰的错误提示,而不是运行到一半才崩。

## fork 后的常见做法

1. 在 `Cargo.toml` 顶部修改 `default = ["standard"]`(或自定义组合),让
   `cargo build`、`cargo install` 都走你需要的版本。
2. 如果有自动更新需求,把发布资产名/仓库地址写进 `upgrade` 子命令的
   CLI 默认值(`--repository`、`--asset`),用户在你的 fork 上跑
   `oxidns upgrade` 就会自动指向你的发布仓库。
