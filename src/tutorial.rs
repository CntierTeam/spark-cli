pub fn render(topic: Option<&str>) -> String {
    match topic.unwrap_or("overview").to_ascii_lowercase().as_str() {
        "views" => VIEWS.into(),
        "filters" => FILTERS.into(),
        "examples" => EXAMPLES.into(),
        "commands" => COMMANDS.into(),
        _ => OVERVIEW.into(),
    }
}

const OVERVIEW: &str = r#"# spark-dump Tutorial — Overview

spark-dump 是 lucko spark-viewer 的 CLI 版：同一套 protobuf schema，
把 .sparkprofile / .sparkheap / .sparkhealth（或 bytebin code）dump 成可读文本或 JSON。

## 快速开始

  spark-dump dump   ./file.sparkprofile
  spark-dump profile ./file.sparkprofile --view flat --top 40
  spark-dump heap   ./file.sparkheap --search Entity
  spark-dump health ./file.sparkhealth
  spark-dump meta   ./file.sparkprofile
  spark-dump raw    ./file.sparkprofile -f json
  spark-dump tutorial views|filters|examples|commands

## 与网页 Viewer 的对应关系

| Web                         | CLI                                      |
|----------------------------|------------------------------------------|
| All view                   | profile --view all                       |
| Flat view + Self/Total     | profile --view flat --flat-mode ...      |
| Sources / plugins / mods   | profile --view sources --sources-mode ...|
| Flame graph                | profile --view flame                     |
| Search bar                 | --search / -s                            |
| Thread filter (mental)     | --thread                                 |
| Refine time windows        | windows + --windows / --window-range     |
| Merge mode                 | --sources-mode merge|separate            |
| Bottom-up                  | --bottom-up                              |
| Metadata / widgets         | meta 或 profile 默认 header              |
| Export raw                 | 保留原文件；raw 输出结构化 JSON          |

更多：`spark-dump tutorial views` / `filters` / `examples`
"#;

const VIEWS: &str = r#"# Views（对齐 spark-viewer）

## summary（dump 默认）
平台元数据 + 线程总览。适合先看 TPS/MSPT/CPU，再决定钻哪条线程。

## all
完整调用树（按时间降序）。可用 --depth / --min-percent 裁剪。

## flat
把相同 class+method+desc 聚合：
  --flat-mode self-time   自用时间（去掉子节点）
  --flat-mode total-time  含子树总时间
  --bottom-up             额外打印父链（对齐 Bottom-up）

## sources
按插件/模组 source 归类（需要 profile 内含 class/method/line sources）。

  spark-dump plugins ./x.sparkprofile
  spark-dump plugins ./x.sparkprofile --thread "Folia Region" --no-detail
  spark-dump profile ./x.sparkprofile --view sources --sources-mode merge

## flame
终端版火焰图（条宽 ≈ 占比）。多线程时用 --flame-thread / --thread 选线程。
"#;

const FILTERS: &str = r#"# Filters

## --search / -s   （Web SearchBar）
子串匹配（忽略大小写）：
  - 线程名
  - className / methodName
  - plugin/mod source
命中后会展开：匹配节点 + 全部祖先 + 全部子孙（与网页一致）。

## --thread
只保留名称匹配的线程（substring；加 --regex 则按正则）。

## --include / --exclude
可重复。过滤 stack 节点的 class/method/source。
  --include ca.spottedleaf --exclude native
  --regex --include '^net\\.minecraft\\.' 

## --min-percent / --depth / --top
树裁剪与 flat/sources 条数限制。

## --top-threads / --min-thread-percent
多线程 profile 时按耗时排序，只 dump 最热的 N 条线程，或丢掉占比过低的线程。

## --windows / --window-range   （Web Refine）
连续 profiling 会切成多个时间窗（约 1 分钟/窗）：

  spark-dump windows ./x.sparkprofile
  spark-dump windows ./x.sparkprofile --top 5 --commands
  spark-dump profile ./x.sparkprofile --windows @12 --view flat
  spark-dump threads ./x.sparkprofile --window-range 10,15

`--windows`：`all` | Refine id | 下标 `@12` / `i12` / `@10,@11`
`--window-range start,end`：按下标闭区间。
"#;

const EXAMPLES: &str = r#"# Examples

# 1) 看整体
spark-dump dump ./JkXT9t07ku.sparkprofile -o summary.txt

# 1b) 多线程列表
spark-dump threads ./JMqWQJ6SpT.sparkprofile --top 20

# 2) Flat self-time Top 30（只看最热 8 条线程）
spark-dump profile ./x.sparkprofile --view flat --flat-mode self-time --top 30 --top-threads 8

# 3) 搜索 tick / scheduler 相关
spark-dump profile ./x.sparkprofile --view all -s tick --min-percent 1 --depth 12

# 4) 只看某插件源
spark-dump profile ./x.sparkprofile --view sources --search MyPlugin

# 5) 排除 native，只要 Java
spark-dump profile ./x.sparkprofile --view flat --exclude native --include java. --top 50

# 6) 火焰图
spark-dump profile ./x.sparkprofile --view flame --thread "Region Scheduler" --min-percent 0.5

# 7) JSON 给脚本吃
spark-dump profile ./x.sparkprofile --view flat -f json -o flat.json

# 8) Heap 搜实体
spark-dump heap ./x.sparkheap -s Entity --top 50

# 9) 只要元数据
spark-dump meta ./x.sparkprofile

# 10) 按时间窗 Refine（连续 profiling）
spark-dump windows ./cRaewtx7pS.sparkprofile --top 10
spark-dump profile ./cRaewtx7pS.sparkprofile --windows @3 --view flat --top 30
spark-dump threads ./cRaewtx7pS.sparkprofile --window-range 0,5
"#;

const COMMANDS: &str = r#"# Commands

spark-dump dump    <file|code>   自动识别类型
spark-dump profile <file|code>   Sampler 全功能
spark-dump windows <file|code>   Refine 时间窗列表
spark-dump threads <file|code>   线程耗时（支持 --windows）
spark-dump heap    <file|code>   Heap 直方图
spark-dump health  <file|code>   Health 报告
spark-dump raw     <file|code>   结构化 JSON
spark-dump meta    <file|code>   仅元数据/widgets
spark-dump plugins <file|code>   插件占用
spark-dump tutorial [topic]

全局常用：
  -o/--out  -f/--format text|json
  -s/--search  --thread  --include  --exclude  --regex
  --depth  --min-percent  --top
  --windows  --window-range   (@N 或 id)
"#;
