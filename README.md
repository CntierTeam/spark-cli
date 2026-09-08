# spark-cli (`spark-dump`)

基于 [lucko/spark-viewer](https://github.com/lucko/spark-viewer) 的 **独立 CLI**（Rust）：把 `.sparkprofile` / `.sparkheap` / `.sparkhealth` dump 成可读文本或 JSON。

仓库：[`CntierTeam/spark-cli`](https://github.com/CntierTeam/spark-cli) · 二进制名：`spark-dump`

对齐网页：All / Flat / Sources（插件占用）/ Flame、过滤器、多线程、Refine 时间窗。

## 一键安装（从 GitHub Release）

```bash
curl -fsSL https://raw.githubusercontent.com/CntierTeam/spark-cli/main/scripts/install.sh | bash
```

指定版本 / 安装目录：

```bash
curl -fsSL https://raw.githubusercontent.com/CntierTeam/spark-cli/main/scripts/install.sh | bash -s -- v1.1.0
INSTALL_DIR=~/bin bash scripts/install.sh latest
```

默认安装到 `~/.local/bin/spark-dump`（Linux 优先 **musl 静态**包）。

Release 资源名：

| Asset | 平台 |
|-------|------|
| `spark-dump-linux-x86_64-musl` | Linux 静态（推荐） |
| `spark-dump-linux-x86_64-gnu` | Linux glibc |
| `spark-dump-linux-x86_64` | musl 别名 |
| `spark-dump-windows-x86_64.exe` | Windows x64 |

## 从源码构建

```bash
cargo build --release
./target/release/spark-dump --help
cargo install --path .
```

## 常用命令

```bash
spark-dump tutorial
spark-dump dump ./file.sparkprofile
spark-dump plugins ./file.sparkprofile --thread 'Folia Region'
spark-dump threads ./file.sparkprofile --top 20
spark-dump profile ./file.sparkprofile --view flat --exclude native --top 40
spark-dump profile ./file.sparkprofile --view flame --thread Folia
```

| 命令 | 用途 |
|------|------|
| `dump` | 自动识别并摘要 |
| `profile` | `--view all\|flat\|sources\|flame\|summary` |
| `plugins` | 插件/模组占用 |
| `threads` | 线程耗时排名 |
| `heap` / `health` / `meta` / `raw` | 堆 / 健康 / 元数据 / JSON |
| `tutorial` | 内置教程 |

过滤器：`-s`、`--thread`、`--include`、`--exclude`、`--regex`、`--top-threads`、`--windows` …  
详见 [`TUTORIAL.md`](./TUTORIAL.md)。

## CI / Release

- **CI**：`.github/workflows/ci.yml` — push/PR 构建 Linux gnu/musl + Windows
- **Release**：`.github/workflows/release.yml` — 推送 `v*` tag（或手动 workflow_dispatch）打包并上传 Release

发版：

```bash
git tag v1.1.0
git push origin v1.1.0
```

## Codex Skill

```bash
# 开发机：用本仓库 skill
./scripts/install-codex-skill.sh copy   # 或 link

# 任意机器：从 GitHub 拉 skill 源码
./scripts/install-codex-skill.sh release
VERSION=v1.1.0 ./scripts/install-codex-skill.sh release
```

安装到 `~/.codex/skills/spark-dump`。Skill 目录：[`skills/spark-dump/`](./skills/spark-dump/)。

## 许可

proto / 解析思路来自 lucko spark-viewer 与 spark2json（MIT）。见 `LICENSE*.txt`。
