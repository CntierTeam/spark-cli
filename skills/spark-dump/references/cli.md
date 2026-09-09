# spark-dump CLI reference

## Subcommands

```text
spark-dump dump <input>       Auto-detect sampler/heap/health
spark-dump profile <input>    Sampler views
spark-dump windows <input>    Refine time-window list (idx/id/share)
spark-dump plugins <input>    Plugin/mod occupancy (Sources)
spark-dump threads <input>    Ranked thread list (--windows supported)
spark-dump heap <input>       Heap histogram
spark-dump health <input>     Health report
spark-dump meta <input>       Metadata / widgets only
spark-dump raw <input>        Structured JSON summary
spark-dump tutorial [topic]   overview|views|filters|examples|commands
```

`input` = local path (`.sparkprofile` / `.sparkheap` / `.sparkhealth`) or bytebin code (needs `curl`).

## profile --view

| Value | Meaning |
|-------|---------|
| `summary` | Meta + ranked threads |
| `all` | Call tree |
| `flat` | Aggregated methods (`--flat-mode self-time\|total-time`, `--bottom-up`) |
| `sources` | Same data as `plugins` |
| `flame` | ASCII flame; defaults to heaviest thread |

## Time windows (Refine)

Continuous profiles split samples into ~1m buckets. List then select:

```bash
spark-dump windows ./x.sparkprofile --top 10
spark-dump profile ./x.sparkprofile --windows @12 --view flat
spark-dump threads ./x.sparkprofile --window-range 10,15
```

`--windows` accepts `all`, Refine ids, or indices (`@12` / `i12` / `@10,@11`).

## plugins columns

| Column | Meaning |
|--------|---------|
| `%thread` | Share of selected threads' total time (includes park/native) |
| `%plugins` | Share among attributed plugin/mod samples only |
| `time` | Absolute sample time units |

`--no-detail` hides per-plugin hot methods. `--sources-mode merge|separate` matches web Merge Mode.

## Useful recipes

```bash
# Orient
spark-dump dump ./x.sparkprofile
spark-dump threads ./x.sparkprofile --top 20

# Plugin lag on Folia region tick
spark-dump plugins ./x.sparkprofile --thread 'Folia Region'

# Hot Java methods, drop native
spark-dump profile ./x.sparkprofile --view flat --thread Folia \
  --exclude native --top 40 --top-threads 5

# Search + tree
spark-dump profile ./x.sparkprofile --view all -s ArenaSession --depth 12 --min-percent 0.5

# JSON for scripts
spark-dump plugins ./x.sparkprofile -f json -o plugins.json
```

## Install (release)

```bash
curl -fsSL https://raw.githubusercontent.com/CntierTeam/spark-cli/main/scripts/install.sh | bash
```

Assets: `spark-dump-linux-x86_64-musl`, `spark-dump-linux-x86_64-gnu`, `spark-dump-windows-x86_64.exe`.
