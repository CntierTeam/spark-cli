---
name: spark-dump
description: >-
  Analyze lucko spark profiler dumps with the spark-dump Rust CLI (.sparkprofile /
  .sparkheap / .sparkhealth). Covers views (all/flat/sources/flame), filters, plugin
  occupancy, multi-thread ranking, Refine time windows, and Codex skill install.
  Trigger on: spark-dump, sparkprofile, sparkheap, spark-viewer, lucko spark,
  plugin occupancy, Sources view, Folia profiler, Minecraft sampler dump, time window.
license: MIT
metadata:
  short-description: lucko spark profile CLI dump & analysis
---

# spark-dump

Standalone Rust CLI that dumps lucko spark binary profiles into human-readable text/JSON.
Schema and analysis semantics align with [lucko/spark-viewer](https://github.com/lucko/spark-viewer).

Binary: `spark-dump` (also shipped under `dist/`).

## Hard rules

1. Prefer the **installed binary** (`spark-dump` on `PATH`, or `dist/spark-dump-linux-x86_64-gnu` / `dist/spark-dump.exe`) over re-implementing protobuf parsing.
2. **Unflatten** `childrenRefs` + sum `times[]` before analyzing (viewer Preprocessing). Never treat the flat `children` array as the call tree.
3. Plugin occupancy = **Sources** map (`classSources` / `methodSources` / `lineSources`), not the metadata plugins list alone.
4. `%thread` vs `%plugins` are different: most Folia time is often park/native → `%thread` looks tiny while `%plugins` ranks plugins among attributed time.
5. Multi-thread profiles: sort by time; use `threads` / `--top-threads` / `--thread` before dumping huge trees.
6. Code/comments in English; user-facing replies follow the user's language.

## Resolve the binary

```bash
# Prefer PATH after install
command -v spark-dump

# Or repo-built artifacts
./dist/spark-dump-linux-x86_64-gnu --help          # Linux dynamic
./dist/spark-dump-linux-x86_64-musl --help          # Linux static (if present)
./dist/spark-dump.exe --help                       # Windows
./target/release/spark-dump --help
```

Build from this repo:

```bash
cargo build --release
# optional cross:
# cargo build --release --target x86_64-pc-windows-gnu
# cargo build --release --target x86_64-unknown-linux-musl
```

## Command map (viewer parity)

| Need | Command |
|------|---------|
| Auto dump | `spark-dump dump <file>` |
| Call tree | `spark-dump profile <file> --view all` |
| Hot methods | `spark-dump profile <file> --view flat --flat-mode self-time` |
| Plugin occupancy | `spark-dump plugins <file>` |
| Sources (same data) | `spark-dump profile <file> --view sources` |
| Flame | `spark-dump profile <file> --view flame --thread '…'` |
| Thread list | `spark-dump threads <file>` |
| Time windows (Refine) | `spark-dump windows <file>` then `--windows @N` |
| Meta/widgets | `spark-dump meta <file>` |
| Heap | `spark-dump heap <file> -s Entity` |
| Health | `spark-dump health <file>` |
| Tutorial | `spark-dump tutorial [views\|filters\|examples]` |

## Filters (always available on profile/plugins/dump)

- `-s/--search` — web SearchBar (class/method/source/thread; expands ancestors+descendants)
- `--thread` — keep matching threads
- `--include` / `--exclude` — stack whitelist/blacklist (repeatable); `--regex` optional
- `--top-threads` / `--min-thread-percent` — multi-thread cut
- `--windows` / `--window-range` — Refine time windows (`@N` index or id; list with `windows`)
- `--depth` / `--min-percent` / `--top` — tree / flat size

## Typical agent workflow

1. Confirm input is `.sparkprofile` / `.sparkheap` / `.sparkhealth` (or bytebin code).
2. `spark-dump dump <file>` or `threads <file>` for orientation; if multi-window, `windows <file> --top 10`.
3. For lag: `plugins <file> --thread 'Folia Region'` then `profile --view flat --thread … --exclude native` (optionally `--windows @N`).
4. Quote **plugin name + %plugins + hot methods**; mention large unattributed/native share when relevant.
5. Prefer `-o out.txt` for large dumps; use `-f json` only when scripting.

## Install this skill into Codex

```bash
./scripts/install-codex-skill.sh              # copy from checkout
./scripts/install-codex-skill.sh link         # symlink
./scripts/install-codex-skill.sh release      # pull from CntierTeam/spark-cli
```

Install the **binary** from GitHub Releases:

```bash
curl -fsSL https://raw.githubusercontent.com/CntierTeam/spark-cli/main/scripts/install.sh | bash
```

Repo: https://github.com/CntierTeam/spark-cli

## References

- CLI details: [references/cli.md](references/cli.md)
- Project tutorial: `TUTORIAL.md`
- Proto: `proto/spark.proto` (from lucko spark-viewer)
