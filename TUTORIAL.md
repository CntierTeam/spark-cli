# spark-dump Tutorial

内置命令：`spark-dump tutorial [overview|views|filters|examples|commands]`

## 1. 它是什么

把 lucko spark 的二进制采样/堆/健康数据，在终端里做出和 [spark-viewer](https://github.com/lucko/spark-viewer) 同类的分析：

- 调用树（All）
- 聚合热点（Flat self/total）
- 按插件/模组归类（Sources）
- 火焰图（Flame）
- 搜索与过滤器（Search / include / exclude / thread）
- 时间窗 Refine（`windows` + `--windows @N` / `--window-range`）

## 2. 子命令一览

| 命令 | 用途 |
|------|------|
| `dump` | 自动识别类型并输出 |
| `profile` | Sampler 全功能视图 |
| `windows` | Refine 时间窗列表（idx / id / 占比） |
| `threads` | 线程耗时排名（支持时间窗） |
| `plugins` | 插件/模组占用 |
| `heap` | Heap 直方图 + 搜索 |
| `health` | Health 报告 |
| `raw` | 结构化 JSON |
| `meta` | 只看 widgets/元数据 |
| `tutorial` | 本教程 |

## 3. Views

### All
```bash
spark-dump profile file.sparkprofile --view all --depth 12 --min-percent 0.5
```

### Flat
```bash
spark-dump profile file.sparkprofile --view flat --flat-mode self-time --top 40
spark-dump profile file.sparkprofile --view flat --flat-mode total-time --bottom-up
```

### Sources
```bash
spark-dump profile file.sparkprofile --view sources --sources-mode merge
spark-dump profile file.sparkprofile --view sources -s MyPlugin
```

### Flame
```bash
spark-dump profile file.sparkprofile --view flame --thread "Region Scheduler"
```

## 4. Filters（对齐网页）

| Flag | 行为 |
|------|------|
| `-s/--search` | 子串匹配 class/method/source/thread；命中后展开祖先+子孙 |
| `--thread` | 线程名过滤 |
| `--include` | 白名单（可重复） |
| `--exclude` | 黑名单（可重复） |
| `--regex` | 上述模式按正则 |
| `--windows` | `all` / Refine id / 下标 `@N` `iN` |
| `--window-range` | 按下标闭区间选窗 |
| `--depth` / `--min-percent` / `--top` | 裁剪 |

先列窗再钻取：
```bash
spark-dump windows x.sparkprofile --top 10 --commands
spark-dump profile x.sparkprofile --windows @3 --view flat --top 40
spark-dump threads x.sparkprofile --window-range 10,15
```

示例：
```bash
spark-dump profile x.sparkprofile --view flat \
  --include ca.spottedleaf --exclude native --top 50
```

## 5. 输出

```bash
spark-dump profile x.sparkprofile --view flat -f text -o out.txt
spark-dump profile x.sparkprofile --view flat -f json -o out.json
```

## 6. 端到端示例

```bash
# 摘要
spark-dump dump ./JkXT9t07ku.sparkprofile | less

# 找 tick 相关热点
spark-dump profile ./JkXT9t07ku.sparkprofile --view flat -s tick --top 30

# 火焰图看调度线程
spark-dump profile ./JkXT9t07ku.sparkprofile --view flame \
  --thread "Folia Region" --min-percent 1 --depth 14
```
