# STATUS — Quota Tray

> 快照：2026-11-13。
> 本文件是**进度快照**，不是契约。契约在 `PLAN.md`，代理规则在 `AGENTS.md`。
> 用途：换新对话 / 换 agent 时，读这一份就能接手，不需要历史对话。

## 一句话

**Phase 1 已完成**：`kimi-planbar-tray/rust/` 基线已落到仓库根目录并改名为 Quota Tray（0.1.0），`docs/SPEC.md` 已随基线拷入。下一个工作是 Phase 2：按 D 版 token 重写面板 UI 并加 tab。

## 已完成

| 阶段 | 产物 |
|---|---|
| P0 | 四版设计探索 + A/B/C/D 并排对比；`demo/panel-demo-d.html`（定稿）；`shot.sh` / `crop_panel.py` / `heights.py` / `check_contrast.py`；`PLAN.md`、`AGENTS.md`；仓库初始化 |
| P1 | 基线导入（`index.html`、`settings.html`、`skills.html`、`menu.html`、`src/`、`vite.config.ts`、`tsconfig.json`、`package*.json`、`src-tauri/`）、全量改名 Quota Tray、`docs/SPEC.md` + `docs/SPEC_EN.md`（首页加合并后适用范围表）、`CHANGELOG.md`（0.1.0 起） |

## Phase 1 做了什么（供核对）

改名覆盖的文件：

| 文件 | 结果 |
|---|---|
| `src-tauri/tauri.conf.json` | `productName` = Quota Tray；identifier = `com.shawnqi.quotatray`；version = 0.1.0；main/settings/menu 三窗标题（skills 窗仍是 `Kimi Skills`） |
| `src-tauri/Cargo.toml` | crate `quota-tray`、lib `quota_tray_lib`、version 0.1.0、新描述 |
| `package.json` / `package-lock.json` | name `quota-tray`、version 0.1.0 |
| `src-tauri/src/main.rs` | 互斥锁 `QuotaTray.SingleInstance`；4 处 `quota_tray_lib::` |
| `src-tauri/src/settings.rs` | 配置目录 `%APPDATA%\QuotaTray`；HKCU Run 值名 `QuotaTray`（2 处） |
| `src-tauri/src/lib.rs` | `QUOTA_TRAY_APP_DATA_DIR`（3 处 + settings.rs 注释 1 处）；兜底退出菜单 `Quit Quota Tray` / id `quotatray-fallback-quit`；`--test-ui` 临时目录前缀 `quota-tray-testui-`；头部注释 |
| `src-tauri/src/tray.rs` | tooltip 与菜单文案（4 处）`Quota Tray  5h … · week …` |
| `src-tauri/src/update.rs` | `User-Agent: QuotaTray` |
| `src-tauri/src/skills.rs` | 测试临时目录前缀 `quotatray-skills-test-` |
| `index.html` / `settings.html` | `<title>` 与标题栏文字 |
| `src/theme.css` | 首行注释 |

未改动、仍待拍板：`src-tauri/icons/`（沿用 kimi 的 K 标）、窗口初始尺寸（424×520，P2 改自适应）、skills 窗标题与皮肤。

## 最近一次自检（2026-11-13，本机）

| 命令 | 结果 |
|---|---|
| `npm install` | 20 包，0 漏洞 |
| `npm run build` | 4 页产物落 `dist/`，标题已是 Quota Tray |
| `npx tsc --noEmit` | 通过，无输出 |
| `cargo fmt --check`（`src-tauri/`） | 干净（改名后有两处被 rustfmt 折行，已 `cargo fmt`） |
| `cargo build` | 通过；`target/debug/quota-tray.exe` 已生成（首编 5m43s） |
| `cargo test` | 5 passed（recovery ×3、skills ×2） |
| `quota-tray.exe --test-fetch` | 真机凭证打通，返回 live JSON，`error: null` |
| `quota-tray.exe --test-ui` | `MainWindow / SettingsWindow / SkillsWindow / TrayMenuWindow OK`，退出码 0；四窗尺寸与标题正确（424×520 / 404×464 / 404×520 / 188×160） |
| 旧 UI 渲染 | `dist` 经本地 http 服务在 Edge headless 424px 下截图：卡片、双读数区、Extra Usage、CLI 行、四个 ghost 按钮均正常，标题已显示 Quota Tray（`--` 占位是因为浏览器里没有 Tauri IPC） |

注：`--test-ui` 会在屏幕上真实弹出四个窗口约 6 秒；调试版 exe 走 Vite devUrl，要看渲染内容需先 `npm run dev`。`--test-deepseek` / `--test-login` 尚未实现（P3/P4）。

## 接手必须知道的六件事

1. **源仓库在本机同级目录**：`~/Documents/trae_projects/github/kimi-planbar-tray`（只用 `rust/`，已导入完毕，后续只作对照）、`~/Documents/trae_projects/github/deepseek-planbar`（只用 `tray-wpf/`，P3/P4 移植用）。两个原仓库保持独立，不改不删。
2. **视觉基准只有一个文件**：`demo/panel-demo-d.html`；数值与 token 在 `PLAN.md` §3。A/B/C 三版只是对照，不要在上面继续改。
3. **本机 Chrome 未安装，只有 Edge**。任何截图/自动化都必须运行时探测（Chrome → Edge → 其他 Chromium），不得写死路径。
4. **`core.autocrlf=true`**，所以仓库有 `.gitattributes` 强制 `eol=lf`。若把 `demo/shot.sh` 之类脚本改回 CRLF，Git Bash 会 `bad interpreter` 直接跑不起来。
5. **提交会过全局 pre-commit 钩子**（gitleaks 8.29.1 + 密钥正则 + 隐私 + 本机路径三层）。文档和代码里不要写本机绝对路径（盘符 + 用户目录的形式会被拦），用户目录一律写 `~`。
6. **改行为先改 `PLAN.md`**，同一提交里完成；`AGENTS.md` 的陷阱清单同步更新。`docs/SPEC.md` 记录的是继承来的 Kimi 行为，与 `PLAN.md` 冲突时 `PLAN.md` 优先。

## 定稿设计速览（细节见 `PLAN.md` §3）

| 维度 | 结论 |
|---|---|
| 结构 | 424 窗口（卡片 368 + 28 阴影留白）· 双行 header（字标 13/500 + 时间戳 11/400，下接全宽分段控件）· 两列裸读数区（无填充/无描边/无圆角，列距 24）· 1px 发丝线分行 · 4 个 ghost 按钮 |
| 字体 | 全系统 sans：`'Segoe UI', system-ui, …`，**零 mono、零 web font**；数字 30/600 + `tabular-nums`；微标签 10/500 大写 `.08em` |
| 颜色 | 暖纸底 `#fbfaf9` / `#181817`；alpha 墨阶（90/60/25%）；**单一 Kimi 蓝**（`#1783ff` / `#1a88ff`，条形用深一档 `#0d6fe8`）；**唯一红**只给故障 |
| 语义 | 蓝 = 用量 · 红 = 故障（余额不足 / token 过期 / Key 失效 / 更新失败 / 解析失败 / 命中率 <60%）· 墨 = 正常（用量百分比永不变红，`Available` 不着色） |
| tab 控件 | 灰底轨道 radius 12 + 等宽格 radius 8 + 活动格 1px 内描边 ring（**无投影**） |
| 高度（实测） | Kimi tab 330 · DeepSeek 2 模型 403 · 4 模型 545（卡片高；窗口高 = +56） |

## 下一步：Phase 2 可执行清单

按 `PLAN.md` §8：

1. 用 D 版 token 重写 `src/theme.css` / `main.css` / `index.html` 结构：双行 header、分段 tab、两列裸读数区、发丝线、ghost footer。
2. `src/common.ts` 的格式化函数对齐 `PLAN.md` §3（`tabular-nums`、10.5px 说明行、`Resets in …`）。
3. tab 状态与 `ActiveTab` 持久化（`settings.json`，`PLAN.md` §7）；窗口高度由前端测量上报、后端 `LogicalSize` + `PhysicalPosition` 右下角钉回（`PLAN.md` §4）。
4. 托盘 tooltip 与面板文案统一切到 D 版；skills 窗皮肤待定（先不动）。
5. 验收：连点 tab 30 次不跳位、窗口底边与右边缘不动；`bash demo/shot.sh …` + `crop_panel.py` 424px、明暗两套与 `panel-demo-d.html` 逐屏比对；`--test-ui` 通过；`heights.py` 仍是 330 / 403 / 545。

## 未决（等你拍板）

1. **tab 活动块的 ring 对比度**：中性灰对轨道只有约 1.2–1.3:1，低于 WCAG 对状态标识的 3:1；可选改成强调蓝 ring（对轨道 3.8:1，代价是标题区多一个蓝元素）。当前保留中性。
2. **skills 窗口**是否随 D 版换肤（现在还是旧皮肤）。
3. **托盘图标**是否换新（当前沿用 kimi 的 K 标）。
4. **GitHub 仓库名 / 是否建 remote**（建议 `quota-tray`）。目前无 remote。

## 新对话开场建议

> 接手 Quota Tray（当前目录已是 git repo）。按顺序读 `STATUS.md` → `AGENTS.md` → `PLAN.md`，视觉基准是 `demo/panel-demo-d.html`。Phase 1 已落地，从上面的 Phase 2 清单开始，完成后按 `PLAN.md` §8 的验收项自检。
