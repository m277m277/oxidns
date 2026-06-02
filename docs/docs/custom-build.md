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

| Bundle | 定位 | 体积参考 |
|---|---|---|
| `minimal` | 小内存设备 / 容器 / 实验，仅基础 UDP/TCP 转发 | 约 8.9 MB |
| `standard` | 家用路由器，含 WebUI + 加密协议 + 常用插件 | 介于两者之间 |
| `full`（默认） | 全功能，覆盖发布版的全部能力 | 约 21 MB |

> fork 之后自行组合 feature 时，实际能力以 `oxidns build-info` 或
> `GET /api/build` 返回值为准。下面每个小节列出的是官方预设的具体内容。

### `minimal` — 最小可用

只编译 DNS 转发必需的部分，剔除 HTTP / 加密协议 / 全部可选插件。
依赖排除了 hyper / rustls / quinn / h2 / h3 / sqlite / zoneparser，是体积最小的组合。

**包含**

- 入站：UDP、TCP
- 上游：UDP、TCP（明文）
- Provider：`domain_set`、`ip_set`
- Matcher：全部
- Executor：`sequence`、`forward`、`cache`、`fallback`、`hosts`、`redirect`、
  `dual_selector`、`ecs_handler`、`ttl`、`drop_resp`、`black_hole`、
  `debug_print`、`reload`
- 进程内 metric 计数器（不暴露 HTTP 端点）

**不包含**

- 管理 API / WebUI / Prometheus `/metrics`
- DoT / DoH / DoQ / DoH3 协议
- 全部可选插件与 `upgrade` 子命令

### `standard` — 家用路由器

`minimal` 之上引入管理面、加密协议栈和常用插件。

**额外包含**

- 管理面：HTTP API、WebUI、Prometheus `/metrics`、`metrics_collector`
- 入站 / 上游：DoT、DoH (HTTP/2)、DoQ
- Provider：`geoip`、`geosite`、`v2ray_dat`、`adguard_rule`
- Executor：`arbitrary`、`cron`、`download`、`http_request`、
  `reverse_lookup`、`query_recorder`、`script`
- `upgrade` CLI 子命令与 `upgrade` executor

### `full`（默认） — 全功能

`standard` 之上引入 DoH3 与平台集成。

**额外包含**

- 入站 / 上游：DoH HTTP/3
- Executor：`ros_address_list`（MikroTik）、`ipset`、`nftset`

### 官方发布物对照

| 渠道 | minimal | standard | full |
|---|---|---|---|
| Linux x86_64 / ARM64 musl slim 包 | ✓（无 WebUI） | ✓（含 WebUI） | — |
| 完整 release target | — | — | ✓ |
| `.deb` / Docker | — | — | ✓ |

## 颗粒度开关

下表里的每个 feature 都可以单独打开或关闭。组合包就是这些开关的集合,
你也可以**只挑自己需要的开关**而不走预设。

### 入站 / 出站协议

| Feature | 作用 |
|---|---|
| `server-dot` | 启用 DoT(TLS over TCP)入站服务器,依赖 rustls 服务端栈 |
| `server-doh` | 启用 DoH(HTTP/2 over TLS)入站服务器,依赖 hyper 服务端 + rustls |
| `server-doq` | 启用 DoQ(QUIC)入站服务器,依赖 `quinn` |
| `server-doh3` | 在 DoH 服务器上启用 HTTP/3 路径(需 `server-doh`),额外依赖 `h3` / `h3-quinn` / `quinn` |
| `upstream-dot` | 启用 DoT upstream(`tls://` scheme),依赖 rustls 客户端栈 |
| `upstream-doh` | 启用 DoH(HTTP/2)upstream(`https://` scheme),依赖 hyper-rustls + `h2` |
| `upstream-doq` | 启用 DoQ upstream(`quic://` / `doq://` scheme) |
| `upstream-doh3` | 启用 DoH HTTP/3 upstream(`h3://` scheme 或 `enable_http3: true`,需 `upstream-doh`) |

> 关闭某个协议后,如果 yaml 里仍写了对应 scheme/配置,启动时会以例如
> "upstream DoT is not compiled into this build; rebuild with --features
> upstream-dot" 报错,而不是崩溃。`server-dot` 关闭时,`tcp_server` 配置里
> 写 `cert` / `key` 会得到 "DoT is not compiled into this build; rebuild with
> --features server-dot" 的清晰提示。

### 管理面

| Feature | 作用 | 依赖 |
|---|---|---|
| `api` | 管理 / 健康 / 控制 / 日志 / 配置 HTTP API,以及各插件的 `/plugins/<tag>/...` 端点 | hyper 服务端 + rustls 服务端(支持 HTTPS) |
| `webui` | 在 API hub 上托管 WebUI 静态资源(依赖 `api`) | — |
| `metrics` | `/metrics` Prometheus 端点 + `metrics_collector` 执行器(依赖 `api`) | — |

> `api` 关闭后,`src/api/` 整个模块都不编译,hyper / rustls 服务端栈随之
> 排除,这是 `minimal` 体积大幅下降的主因。进程内的 `MetricSource` 计数器
> 始终保留在 core,关闭 `metrics` 只是不暴露 HTTP 端点,不影响热路径。
> `AppController` / `LogBuffer` 已经下沉到 `src/core/`,因此核心运行时(重载、
> 关停、日志环形缓冲)在没有 `api` 的 `minimal` 构建里依然可用。

### 可选插件

| Feature | 插件 | 主要依赖 |
|---|---|---|
| `plugin-mikrotik` | `ros_address_list` | `mikrotik-rs` |
| `plugin-query-recorder` | `query_recorder` | `rusqlite`(bundled SQLite) |
| `plugin-ipset` | `ipset` + `nftset` | `ripset`(Linux only) |
| `plugin-cron` | `cron` | `cronexpr` |
| `plugin-script` | `script` | — |
| `plugin-arbitrary` | `arbitrary` | `oxidns-zoneparser` |
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

# 家用路由器(含 API、DoT/DoH/DoQ、geo、adguard 等常用 provider/executor)
cargo build --release --no-default-features --features standard

# 只在 minimal 上加 MikroTik 集成
cargo build --release --no-default-features --features "minimal,plugin-mikrotik"

# 只要纯转发 + 管理 API,不要任何重型插件
cargo build --release --no-default-features --features "minimal,api"
```

官方 release 默认产物仍是 `full`。Linux x86_64 / ARM64 musl 额外提供
`minimal` / `standard` 精简压缩包,名称形如
`oxidns-standard-x86_64-unknown-linux-musl.tar.gz`。`minimal` 包只包含
二进制、默认配置和许可证；`standard` 包额外包含 WebUI 静态文件,并内置
`query_recorder` 与 `upgrade` 子命令。

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
   CLI 默认值(`--repository`、`--asset` / `--bundle`),用户在你的 fork 上跑
   `oxidns upgrade` 就会自动指向你的发布仓库。custom 构建不应依赖
   `bundle: auto`,建议显式设置 `asset`。
