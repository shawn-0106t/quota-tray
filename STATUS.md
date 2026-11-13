# STATUS — Quota Tray

> 快照：2026-11-13。
> 本文件是**进度快照**，不是契约。契约在 `PLAN.md`，代理规则在 `AGENTS.md`。
> 用途：换新对话 / 换 agent 时，读这一份就能接手，不需要历史对话。

## 一句话

设计已定稿（D 版）、仓库已初始化、文档齐备；**源代码尚未落地**。下一步是 Phase 1：把 `kimi-planbar-tray/rust/` 拷进本仓库根目录并改名 Quota Tray。

## 已完成

| 项 | 产物 |
|---|---|
| 四版设计探索 + A/B/C/D 并排对比 | `demo/panel-demo-d.html`（**定稿**）、`demo/panel-demo-a/b/c.html`（存档，不再演进）、`demo/compare.html` |
| 设计校验工具（零依赖） | `demo/shot.sh`（截图，Chrome→Edge 探测）、`demo/crop_panel.py`、`demo/heights.py`（四版×两 tab×五状态高度）、`demo/check_contrast.py`（53 对 WCAG，当前全过） |
| 设计 + 行为契约 | `PLAN.md`（11 节：决策记录、D 版 token/几何、窗口矩阵、数据契约、漂移加固、平台行为、阶段与验收、风险） |
| 代理规则 | `AGENTS.md` |
| 仓库 | `git init -b main`；`.gitignore`、`.gitattributes`（强制 LF）、MIT `LICENSE`、`NOTICE` |

提交历史（`main`，**无 remote**）：

```
e96f813  docs(plan): adopt the official /models endpoint for DeepSeek display names
c7c5077  chore: initialize repo with frozen design demos, plan and agent guidance
```

## 接手必须知道的六件事

1. **源仓库在本机同级目录**：`~/Documents/trae_projects/github/kimi-planbar-tray`（只用 `rust/`）、`~/Documents/trae_projects/github/deepseek-planbar`（只用 `tray-wpf/`）。远程：`github.com/shawn-0106t/kimi-planbar-tray`、`github.com/shawn-0106t/deepseek-planbar`。两个原仓库保持独立，不改不删。
2. **视觉基准只有一个文件**：`demo/panel-demo-d.html`；数值与 token 在 `PLAN.md` §3。A/B/C 三版只是对照，不要在上面继续改。
3. **本机 Chrome 未安装，只有 Edge**。任何截图/自动化都必须运行时探测（Chrome → Edge → 其他 Chromium），不得写死路径。
4. **`core.autocrlf=true`**，所以仓库有 `.gitattributes` 强制 `eol=lf`。若把 `demo/shot.sh` 之类脚本改回 CRLF，Git Bash 会 `bad interpreter` 直接跑不起来。
5. **提交会过全局 pre-commit 钩子**（gitleaks 8.29.1 + 密钥正则 + 隐私 + 本机路径三层）。文档和代码里不要写本机绝对路径（盘符 + 用户目录的形式会被拦），用户目录一律写 `~`。
6. **改行为先改 `PLAN.md`**，同一提交里完成；`AGENTS.md` 的陷阱清单同步更新。

## 定稿设计速览（细节见 `PLAN.md` §3）

| 维度 | 结论 |
|---|---|
| 结构 | 424 窗口（卡片 368 + 28 阴影留白）· 双行 header（字标 13/500 + 时间戳 11/400，下接全宽分段控件）· 两列裸读数区（无填充/无描边/无圆角，列距 24）· 1px 发丝线分行 · 4 个 ghost 按钮 |
| 字体 | 全系统 sans：`'Segoe UI', system-ui, …`，**零 mono、零 web font**；数字 30/600 + `tabular-nums`；微标签 10/500 大写 `.08em` |
| 颜色 | 暖纸底 `#fbfaf9` / `#181817`；alpha 墨阶（90/60/25%）；**单一 Kimi 蓝**（`#1783ff` / `#1a88ff`，条形用深一档 `#0d6fe8`）；**唯一红**只给故障 |
| 语义 | 蓝 = 用量 · 红 = 故障（余额不足 / token 过期 / Key 失效 / 更新失败 / 解析失败 / 命中率 <60%）· 墨 = 正常（用量百分比永不变红，`Available` 不着色） |
| tab 控件 | 灰底轨道 radius 12 + 等宽格 radius 8 + 活动格 1px 内描边 ring（**无投影**） |
| 高度（实测） | Kimi tab 330 · DeepSeek 2 模型 403 · 4 模型 545（卡片高；窗口高 = +56） |

## 下一步：Phase 1 可执行清单

1. **拷入基线**（排除 `node_modules/`、`dist/`、`src-tauri/target/`、`src-tauri/gen/`）：
   `index.html settings.html skills.html menu.html src/ vite.config.ts tsconfig.json package.json src-tauri/`
2. **改名**（11 个文件，逐条）：

   | 文件 | 位置 | 改成 |
   |---|---|---|
   | `src-tauri/tauri.conf.json` | `productName` | `Quota Tray` |
   | 同上 | `identifier` | `com.shawnqi.quotatray` |
   | 同上 | `version` | `0.1.0` |
   | 同上 | 3 处窗口 `title` | `Quota Tray` / `Quota Tray Settings` / `Quota Tray Menu` |
   | `src-tauri/Cargo.toml` | `name` / `description` / `[lib] name` | `quota-tray` / 新描述 / `quota_tray_lib` |
   | `package.json` | `name` | `quota-tray` |
   | `src-tauri/src/main.rs` | 互斥锁字面量 | `QuotaTray.SingleInstance` |
   | 同上 | 4 处 `kimi_planbar_tray_lib::…` | `quota_tray_lib::…` |
   | `src-tauri/src/settings.rs` | 配置目录 `join("KimiPlanbarTray")` | `join("QuotaTray")` |
   | 同上 | HKCU Run 值名（2 处） | `QuotaTray` |
   | `src-tauri/src/lib.rs` | `KPT_APP_DATA_DIR`（lib.rs 3 处 + `settings.rs` 注释 1 处）、原生兜底菜单文案、头部注释 | `QUOTA_TRAY_APP_DATA_DIR` / `Quit Quota Tray` |
   | `src-tauri/src/tray.rs` | tooltip 与菜单文案（4 处） | `Quota Tray  5h {} · week {}` 等 |
   | `src-tauri/src/update.rs` | `User-Agent` 头 | `QuotaTray` |
   | `index.html` / `settings.html` | `<title>` 与标题栏文字 | `Quota Tray` / `Quota Tray Settings` |
   | `src/theme.css` | 首行注释 | Quota Tray |

   顺带确认：`src-tauri/icons/` 保持不动（图标换不换是未决项）；窗口尺寸先维持 424×520，P2 再改自适应。

3. **拷入文档**：`docs/SPEC.md` + `docs/SPEC_EN.md`（kimi 的行为规格，仍有效），在首页加一段"合并后适用范围"（哪些章节仍生效）；起一个空 `CHANGELOG.md`，从 0.1.0 开始记。
4. **构建与自检**：`npm install` → `npm run build` → `npx tsc --noEmit` → `cargo build`（在 `src-tauri/`）→ `--test-fetch` / `--test-ui` 全绿。
5. **提交**：`chore: import kimi-planbar-tray rust baseline as Quota Tray`。

## Phase 2 的验收数字

- `demo/heights.py` 当前输出（定稿目标）：Kimi 330 / DS 403 / DS 4 模型 545 卡片高。
- 截图对照：`bash demo/shot.sh …` + `crop_panel.py`，424px、明暗两套，与 `panel-demo-d.html` 逐屏比对。
- 交互：连点 tab 30 次不跳位、窗口底边与右边缘不动；失焦隐藏、300ms 重入保护、滑入动画保持原样。

## 未决（等你拍板）

1. **tab 活动块的 ring 对比度**：中性灰对轨道只有约 1.2–1.3:1，低于 WCAG 对状态标识的 3:1；可选改成强调蓝 ring（对轨道 3.8:1，代价是标题区多一个蓝元素）。当前保留中性。
2. **skills 窗口**是否随 D 版换肤（现在还是旧皮肤）。
3. **托盘图标**是否换新（当前沿用 kimi 的 K 标）。
4. **GitHub 仓库名 / 是否建 remote**（建议 `quota-tray`）。目前无 remote。

## 新对话开场建议

> 接手 Quota Tray（当前目录已是 git repo）。按顺序读 `STATUS.md` → `AGENTS.md` → `PLAN.md`，视觉基准是 `demo/panel-demo-d.html`。从 STATUS 里的 Phase 1 清单开始，完成后按 PLAN §8 的验收项自检。
