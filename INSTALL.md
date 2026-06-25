# 安装 octos（Windows / Linux / macOS）

本页只讲**安装 octos 本体**。装好后再按
[deploy-to-octos.sh](deploy-to-octos.sh) / [END-TO-END-DEMO.md](END-TO-END-DEMO.md)
把飞特小车 skill 挂上。

> 这些是 octos **官方 release**（`octos-org/octos`）。本仓库（`octos-dora-demo`）
> 是 skill，不是 octos 本体。

## TL;DR — 最简单

| 平台 | 一条命令 |
|------|---------|
| **macOS / Linux** | `curl -fsSL https://github.com/octos-org/octos/releases/latest/download/install.sh \| bash` |
| **Windows (PowerShell)** | `irm https://github.com/octos-org/octos/releases/latest/download/install.ps1 \| iex` |
| **任意平台（已装 Node）** | `npm install -g @octos-org/octos` |

装完验证：

```bash
octos --version
octos --help
```

---

## 方式 A：一键安装脚本（推荐，无前置依赖）

会下载二进制 + 自带 skills，并把 `octos serve` 装成后台服务、起本地面板
`http://localhost:8080/admin/`。

**macOS / Linux**
```bash
curl -fsSL https://github.com/octos-org/octos/releases/latest/download/install.sh | bash
```

**Windows（PowerShell）**
```powershell
irm https://github.com/octos-org/octos/releases/latest/download/install.ps1 | iex
```

常用可选参数（脚本被保存到 `~/.octos/bin/`，可再次调用）：
```bash
~/.octos/bin/install.sh --doctor        # 自检
~/.octos/bin/install.sh --uninstall     # 卸载
# Windows: & "$HOME\.octos\bin\install.ps1" -Doctor / -Uninstall
```

## 方式 B：包管理器（只装二进制，不装服务，需自己跑 `octos serve`）

```bash
# Homebrew（macOS Apple Silicon / Linux x86_64 + ARM64）
brew install octos-org/tap/octos

# npm（Windows / Linux / macOS 通用，需 Node.js）
npm install -g @octos-org/octos
```

两者都装"完整 release 包"（octos 本体 + 自带 skills），但**不**配后台服务——
自己运行 `octos serve` 或 `octos chat`。

## 方式 C：从源码编译（任意平台，需 Rust toolchain）

要改代码 / 跑本 demo 的真车链路时用这个（我们本机就是这么跑的）：

```bash
git clone https://github.com/octos-org/octos
cd octos
cargo install --path crates/octos-cli \
  --features "api,telegram,discord,whatsapp,feishu,twilio,wecom,wecom-bot,audio_mp3"
```

- `api` 是 `octos serve`（Web 面板 / Matrix appservice）**必需**的 feature。
- 也可只 `cargo build`（产物在 `target/debug/octos` 或加 `--release`）直接用，
  不装到 PATH。
- 前置：Rust ≥ 1.85（edition 2024）。Windows 建议装好 MSVC 工具链；纯 Rust TLS
  （rustls），不需要 OpenSSL。

## 选哪个？

| 你的情况 | 选 |
|---------|----|
| 最省事、要后台服务+面板 | **A**（install.sh / install.ps1） |
| 一条命令、三平台统一、已有 Node | **B**（npm） |
| mac/Linux 习惯 brew | **B**（brew） |
| 要改代码 / 跑真车 demo | **C**（源码 cargo） |

## 装完下一步

1. 部署小车 skill：
   ```bash
   git clone https://github.com/ZhangHanDong/octos-dora-demo
   cd octos-dora-demo
   bash deploy-to-octos.sh feetech "$HOME/.octos"
   ```
2. 配 LLM key（如 `MOONSHOT_API_KEY`）、profile、串口，按
   [END-TO-END-DEMO.md](END-TO-END-DEMO.md) 跑通 robrix → octos → 真车。

> Windows 注意：octos 本体可在 Windows 跑，但本 demo 的 robrix(Matrix 客户端) +
> palpo(homeserver) + 串口接线主要在 macOS/Linux 验证过。Windows 上串口设备名是
> `COM3` 之类（用 `OMNI_CAR_PORT=COM3`）。
