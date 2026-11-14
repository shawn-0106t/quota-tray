# PLAN — Quota Tray

> 状态：**设计定稿，P1–P3 与 P2 剩余部分（2026-11-14）已落地**。
> 视觉基准：`demo/panel-demo-e.html`（唯一准绳；`demo/compare.html` 保留 A/B/C/D 四版仅作对照，不再演进）。
> 本文件是行为与设计的契约。改动设计或行为前先改这里，再改代码。

---

## 1. 目标与非目标

**目标**：把一个 Kimi Code 额度托盘应用（`kimi-planbar-tray`）和一个 DeepSeek 余额/用量托盘应用（`deepseek-planbar`）合成**一个** Windows 托盘程序：一个托盘图标、一个面板、用 tab 在 Kimi / DeepSeek 之间切换、一个设置窗、一个设置文件。

**非目标**：
- 不做多平台（仅 Windows 10/11）。
- 不做历史曲线、导出、告警推送。
- 不复刻原仓库的 qt/ egui/ wpf/ 三个版本；本仓库只有 Tauri 一条实现线。
- 不改原仓库；两个原仓库保持独立发布。

## 2. 决策记录（已定，不再讨论）

| 决策 | 结论 | 理由 |
|---|---|---|
| 技术栈 | Tauri 2 + Rust 后端 + vanilla TS/HTML/CSS | kimi 的体验（滑入动画、失焦隐藏、DPI 规则、WebView2 recovery）只存在于其 `rust/` 版；单 exe ~6MB，WebView2 系统自带，DeepSeek 的登录捕获本来也依赖它 |
| 代码基线 | 复制 `kimi-planbar-tray/rust/` 到本仓库根目录 | 直接继承已验证的窗口/托盘/recovery 逻辑，不重写 |
| 视觉方向 | E 版：C 的结构 + A 的字体 + A 的配色（收敛到两色），强调色为自定静紫 | 结构上 C 的两行 header + 无框读数区最清爽；字体上 A 的全 sans 比 mono 更耐看；配色上 A 的暖纸底 + alpha 墨阶，砍掉多余色相；2026-11-14 在 D 之上换强调色并给 tab 加紫 ring（见 §3.4/§3.5） |
| 字体 | 全系统字体，零下载；**不用 Inter** | `kimi-planbar-tray` 本身就是 `'Segoe UI', system-ui, sans-serif` 一条声明；Windows 小字号下 Segoe UI 的 hinting 最优，Inter 需打包字体且与 CJK 混排基线不一致 |
| 数字字体 | sans + `tabular-nums`，**不使用 mono** | 等宽数字的"代码感"与托盘工具的定位不符；`tabular-nums` 已能保证刷新不跳宽 |
| 强调色 | 单一静紫（`#8c74b1` / dark `#9d88bc`），两个 tab 共用 | 2026-11-14 用户决定：弃用 Kimi 蓝，换成自定的低彩度紫（取 CLI `gentle` 主题 primary 的色相 264°、彩度 28%，重算明度阶梯）。仍是唯一功能强调色，DeepSeek 品牌蓝、成功绿、警告琥珀全部取消，红只留给真实故障 |
| tab 控件 | 灰底轨道 + 等宽单元格 + 活动块用 1px 内描边 ring（**不用投影**） | 去投影后更平面；ring 比阴影更省视觉噪音 |
| 合并版仓库 | 新建独立 repo，两个原仓库不动 | 原仓库有各自的 CI/发布线，合并进去会污染语义 |
| 产品名 | **Quota Tray**；`com.shawnqi.quotatray`；`quota-tray.exe` | — |
| 版本 | 0.1.0 起 | — |
| DeepSeek 登录捕获 | 进 v1 | 与现版功能对齐；代价是 WebView2 COM 互操作是本项目风险最高的一块 |
| DeepSeek 凭证存储 | `config_dir()/deepseek-credentials.json`，值用 Windows DPAPI 加密（`dpapi:` 前缀），旧版文件首次运行一次性导入 | API Key 能花钱，不该落明文盘；存储跟着 `config_dir()` 走，便携模式才带得走；旧文件只读不写，旧应用不受影响 |

## 3. 设计定稿（E 版）

### 3.1 结构骨架

```
┌─ 424px 窗口（含 28px 阴影留白），卡片 368px，圆角 12 ─┐
│ Quota Tray                              Updated 18:14 │  ← 13/500 + 11/400
│ ┌───────────────────────────────────────────────────┐ │
│ │      Kimi        │        DeepSeek                │ │  ← 分段控件：轨道 + 等宽格
│ └───────────────────────────────────────────────────┘ │
│ WEEKLY              │  5-HOUR                        │  ← 10/500 大写 .08em
│ 12 %                │  31 %                          │  ← 30/600 + 15/500
│ ▬▬▬▬░░░░░░░░░░      │  ▬▬▬▬▬▬░░░░░░░░                │  ← 4px 胶囊
│ Resets in 2d 18h    │  Resets in 3h 33m              │  ← 10.5/400
├───────────────────────────────────────────────────────┤  ← 1px hair
│ Extra usage                                  No data  │
├───────────────────────────────────────────────────────┤
│ Kimi Code CLI                       0.39.1  (Update)  │
├───────────────────────────────────────────────────────┤
│   Console     Refresh     Settings      Exit          │  ← 4 个 ghost 按钮
└───────────────────────────────────────────────────────┘
```

DeepSeek tab 用同一套骨架：`BALANCE` + `Available` / `¥66.50` / `Granted … Topped up …` → 每模型一行（名称 + 金额 / 占比条 / `1.33M tok · cache 80%` + `today ¥0.30`）→ 合计行（`This month ¥3.00 · 2.45M tok` + `today ¥0.30`）。用量失败时模型行整块换成一行提示（`.hint`），合计行隐藏；余额块与模型列表各自独立报错（§5.2）。

header 右上角的时间戳属于**当前 tab 的 provider**：切 tab 时一起换（`Updated 18:14` / `Fetch failed` / `Not configured`）。跨天缓存的 DeepSeek 数据显示 `Updated M/D HH:MM`，避免把昨天的时间读成今天。

**关键几何**：Kimi 读数区是**两列裸文本**（无填充、无描边、无圆角），列间距 24px，标签与下方行标签左对齐到面板 14px 内边距。

### 3.2 色彩 token

| token | light | dark | 用途 |
|---|---|---|---|
| `--stage` | `#e7e7e4` | `#0f0f12` | demo/预览底色，运行时透明 |
| `--paper` | `#fbfaf9` | `#181817` | 面板底 |
| `--ink-1` | `#000000e6` | `#ffffffd6` | 主文字、数字 |
| `--ink-2` | `#00000099` | `#ffffff8f` | 次文字、标签、说明（10–12px 也达标） |
| `--ink-3` | `#0000004d` | `#ffffff42` | **仅图标/装饰**，不可用于文字 |
| `--hair` | `#00000021` | `#ffffff1f` | 分区发丝线 |
| `--hair-soft` | `#00000012` | `#ffffff12` | 进度条轨道 |
| `--accent` | `#8c74b1` | `#9d88bc` | 强调紫（选中态之外的强调基准）：浅 3.84:1、深 5.65:1 对纸面 |
| `--accent-bar` | `#7d61a6` | `#9d88bc` | 进度条填充、focus ring（对轨道 4.17 / 4.68，≥3:1） |
| `--accent-ink` | `#6c5293` | `#b5abc4` | chip 文字（对 tint 5.04 / 6.55，≥4.5:1）；彩度低于条（28%→18%），小字更干净 |
| `--tint` | `#8c74b12e` | `#9d88bc26` | chip 底。浅色用 **18%** 而非 10%：10% 在纸面上只有 1.12 的提亮，和深色的 1.24 不成对，chip 会显得没上色 |
| `--danger` | `#b42318` | `#ff6b6b` | 故障红 |
| `--seg-track` | `#f1f0ec` | `#262624` | 分段控件轨道；footer hover 底 |
| `--seg-on` | `#ffffff` | `#3a3a37` | 活动格底 |
| `--seg-ring` | `#7d61a6` | `#9d88bc` | 活动格内描边 = 强调色。原来的中性发丝线对轨道只有 1.16 / 1.82，不满足 1.4.11；换紫后 4.46 / 4.82，面板 tab 与设置窗分段控件共用同一个值 |
| 阴影 | `0 12px 40px #00000026` | `0 12px 40px #00000073` | 面板悬浮阴影（case 的 menu 档） |

### 3.3 排版

- 字族：`'Segoe UI', system-ui, -apple-system, 'PingFang SC', 'Microsoft YaHei UI', 'Microsoft YaHei', sans-serif`（与 `kimi-planbar-tray` 的 rust 前端同一条）。
- 全局 `font-variant-numeric: tabular-nums`。
- 层级：字标 13/500 · 时间戳 11/400 · tab 12.5（未选 400 / 选中 500）· 微标签 10/500 大写 `letter-spacing: .08em` · 主数字 30/600 · 单位 15/500 · 行标签 12/400 · 行值 13/600 · 说明 10.5/400 · footer 10.5/500 · chip 9.5/500。
- 行值里的纯文字（`No data` / `Not activated`）用 13/500，不继承 600。

### 3.4 组件规格

| 组件 | 规格 |
|---|---|
| 分段控件 | margin `10 0 12`，padding 3，轨道 radius 12；格高 30、radius 8、`flex: 1`；活动格 `--seg-on` + `inset 0 0 0 1px --seg-ring`。**`--seg-ring` 就是强调色**（2026-11-14 起）：填充差只有 1.14 / 1.33，靠它补到 4.46 / 4.82 才过 1.4.11。面板 tab 与设置窗分段控件是同一个组件、同一个 token，不允许再出现「只靠填充差」的第三处；面板侧只多一个 `.seg.provider` 的 margin，标记仍是 `<label><input type=radio>` + `.seg-item`（键盘用方向键切换） |
| 读数列 | `grid-template-columns: 1fr 1fr; gap: 24px`，列内 padding `10 0 11` |
| 进度条 | 高 4、radius 2、轨道 `--hair-soft`、填充 `--accent-bar`、`margin-top: 9`；模型占比条高 3、`margin-top: 8` |
| 分行 | `border-top: 1px --hair`，padding `9 0`；**最后一个子块** `padding-bottom: 10`（父容器不再加底部内边距，否则该行会光学偏上） |
| DeepSeek 余额块 | padding `2 0 10`；`BALANCE` 标签与右侧状态（11/400 `--ink-2`，`Available` 不着色、`Insufficient balance` 用红）同一行；大数字沿用 30/600，`is-danger` 时转红；说明行 `Granted … · Topped up …` |
| DeepSeek 模型行 | 与普通行同高（`9 0`），行间 hair、末行不画底线；名称 12.5/500 单行省略 + `title`（官方名可能很长），金额 15/600 右对齐不参与省略；占比条用模型 token 份额（`monthTokens / 本组最大值`，非金额份额）；meta 行 `token · cache 命中率` 与 `today` 分列两端 |
| DeepSeek 合计行 | hair 顶线，padding `8 0 10`，10.5/400 `--ink-2`，左右两列 |
| 提示行 `.hint` | 顶线 + padding `10 0`；11/400/1.5 `--ink-2`；故障态用 `--danger`。它可能单独收尾整块，所以自带底部内边距（本表「分行」那条规则） |
| chip | **tag 形状**：radius 4（不是胶囊）、padding `2 8`、`--tint` 底 + `--accent-ink` 字；深浅两主题都要真的看出底色，浅色因此用 18% |
| footer | `grid 4`，`gap 2`，padding 5，按钮高 42、radius 8、图标 16、hover 底 `--seg-track` |
| 开关 | 轨道 38×20、radius 9999、1px `--ink-2` 描边；钮 14、inset 2；开启态轨道 `--ink-1`、钮 `--paper`、位移 18px；150ms 过渡；整行是点击目标；**单色，不占强调色**（强调色只给用量） |
| 焦点 | `outline: 2px solid --accent-bar; outline-offset: 2px` |

### 3.5 颜色语义

- **强调色 = 用量 + 选中态**。只有两个角色：用量（进度条、chip）与「这一格被选中」（分段控件活动格的 ring）。色相是实现选择，不是规则本身：2026-11-14 起是静紫 `#8c74b1` / `#9d88bc`（原来是 Kimi 蓝）。除此之外强调色不得出现——不做"高用量变色"，不给开关、勾选、按钮上色。
- **红 = 故障**（余额不足、token 过期、Key 失效、更新失败、解析失败、命中率 <60%）。
- **墨 = 一切正常**。`Available` 不着色，用量百分比不着色。

### 3.6 动效

- 面板开场：opacity 160ms linear + translateY 16→0 220ms `cubic-bezier(.215,.61,.355,1)`；收场 130/160ms，结束后回调后端真正隐藏（沿用 kimi 的 `finish_hide` 语义）。
- tab 切换：内容 120ms 淡入，窗口高度同帧变更（底部锚定）。实现是一次「入场 pane 淡入」而不是真正的交叉淡入：交叉淡入要把两个 pane 叠在同一格里，而那一格量出来是**较高**的那个 pane，高度就会按没显示的那个 tab 算，底部锚定跟着跳。淡入期间只有 opacity 变，`offsetHeight` 不受影响，实测高度与内容同帧。
- 全局 `prefers-reduced-motion` 同时关掉过渡与动画。

## 4. 窗口与信息架构

| 窗口 | 尺寸 | 定位 | 说明 |
|---|---|---|---|
| `main` | 424 × 自适应（典型 330–545 + 56 留白） | 工作区右下角 12px | 无边框、透明、置顶、不占任务栏、启动时隐藏 |
| `settings` | 404 × 自适应（内容上报，上限工作区 90%；当前约 347） | 屏幕居中 | 分区：通用 / Kimi 只读信息 / DeepSeek 凭证 |
| `skills` | 404 × 520 | 屏幕居中 | Kimi 专属、只读、零后台开销 |
| `menu` | 188 × N | 托盘光标处 | Open / Refresh / Settings / Skills / Exit |
| `login`（DeepSeek） | ~900 × 680 | 居中 | 临时窗，不进 recovery 集合 |

面板高度由前端测量内容后上报（`panel_height`），后端用 `LogicalSize` 改高 + `PhysicalPosition` 把窗口**右下角钉回原位**，并按工作区高度夹取；`show_panel` 先按当前 tab 定尺寸再显示，避免可见跳变。tab 选择持久化到 settings.json 的 `ActiveTab`：切换时前端调 `set_active_tab`，后端只改这一个字段并落盘，不碰两个 poller、调度与 last-good 槽位。两个 pane 只有活动那个参与布局（另一个 `hidden`），所以上报的高度天然是当前 tab 的，后端不需要按 tab 记住两套尺寸。

设置窗同样内容自适应（`settings_height`），首次打开（或高度变化时）按工作区居中，同高度重复打开只钉尺寸、不动位置——用户拖动过标题栏的位置不会被重置；以后加分区（P4 的 DeepSeek 凭证）不需要再写死高度。

已知取舍：窗口 = 卡片 + 两侧 28px 阴影留白，卡片距工作区角 12px，因此窗口本身会越出工作区角 16px。那条是透明但仍参与 hit-test 的窄带，面板打开时压住任务栏右下角的一小块点击区。改法是让卡片内缩到 28px，与「12px 间距」冲突，故保留并记录在此。

## 5. 数据契约

### 5.1 Kimi

- 端点：`GET https://api.kimi.com/coding/v1/usages`（Bearer）。
- 凭证链：`~/.kimi-code/credentials/kimi-code.json` 的 `access_token`（`expires_at` 须 > now + 30s）→ 回退 `~/.kimi-code/config.toml` 中 `base_url` 含 `api.kimi.com/coding` 的 `[providers.*]` 的 `api_key`；`KIMI_CODE_HOME` 覆盖家目录。
- **Extra Usage 单位**：`boosterWallet.balance.amountLeft` 是 **1e-8 元**，转分用 `(raw + 500000) / 1000000`；`priceInCents` 已是分；所有 JSON 数字按字符串建模（容忍数字型）。
- `isEnabled=false` 时 `amountLeft` 是估算值，整张卡显示 `Not activated`。
- CLI 版本：`kimi --version`；新版徽章来自官方 changelog（GitHub API 兜底）。

### 5.2 DeepSeek

- 余额：`GET https://api.deepseek.com/user/balance`（Bearer API Key）。
- 模型元数据（**仅供展示名**，不参与用量口径）：`GET https://api.deepseek.com/models`（Bearer 同一 API Key）；回落顺序与缓存策略见 §6。
- 用量：`GET https://platform.deepseek.com/api/v0/usage/{amount,cost}?month=&year=`（Bearer userToken + `x-app-version: 1.0.0` + Chrome UA + `Accept: */*`）。
- 凭证链：`DEEPSEEK_API_KEY` / `DEEPSEEK_USER_TOKEN` 环境变量优先（命中即用，不读也不写文件）→ `config_dir()/deepseek-credentials.json` → 首次运行时从旧版 `~/.deepseek-planbar/credentials.json` **一次性导入**（旧文件只读不写）→ 未配置。
  文件格式 `{"v":1,"apiKey":"dpapi:<base64>","userToken":"dpapi:<base64>","importedFrom":"~/.deepseek-planbar/credentials.json"}`：**键名明文、值加密**，值用 Windows DPAPI（`CryptProtectData` + `CRYPTPROTECT_UI_FORBIDDEN`）逐字段保护。
  读取按前缀判断：带 `dpapi:` 的解密，不带的按明文接受（手工调试与「退回明文」的逃生口，结构不用改）。解密失败按「未配置」处理，`--test-deepseek --verbose` 必须区分 `credential source: env | file | imported | none | undecryptable`，且**永不打印密钥**。
  有意的取舍：密钥绑当前 Windows 账号，便携盘插到别的账号下解不开（重填即可）；迁移期新旧两版不再共用凭证（旧版读不了密文，新版也不再回写旧文件）。
- 缓存：`config_dir()/deepseek-cache.json`（原子写 temp + rename，失败保留上次成功值）。
- 错误码：`key-invalid` / `token-expired` / `rate-limited` / `server-error` / `http-N` / `network-error` / `parse-error`；余额与用量两类错误互相独立。
- 解析：`data.biz_data` 既可能是对象也可能是数组（取 `[0]`）；`total[]` 按模型聚合，`REQUEST` 计请求不计 token，`PROMPT_CACHE_HIT/MISS_TOKEN` 算命中率；`days[]` 只取本地今天；金额保 double 精度、token 取整（`round_ties_even`，即 .NET `Math.Round` 的默认中点舍入）；**金额按 1e-8 元量子归整**（与本页钱包 `amountLeft` 同一单位）—— 逐条 f64 求和的末位漂移会让面板显示 `24.279870719999998` 而不是 `24.27987072`，归整后与旧版 decimal 逐位一致。过滤全零占位与取前 4 由 `visible_models` 在 fetch 边界执行，见下方「展示口径」最后一条。
- 阈值：命中率 <60% 标红（不含琥珀档）；余额 < ¥10 或 `is_available=false` 标红。两个判定只在**前端**（`common.ts` 的 `isLowCacheHit` / `isLowBalance`）存在——它们只决定颜色、不影响数据，放进 Rust 就会成为没有调用方的死代码。
- 展示口径：
  - 金额一律**两位小数**（`¥3.00` / `¥0.00` / `¥49.52`；2026-11-14 用户决定）：旧版 `FmtMoney` 的「整元省略小数」不再沿用，Kimi 的 `fmtYuan(cents)` 同步改成两位，避免同一面板里出现两种写法。DeepSeek 的单位是**元**、Kimi 的是**分**，两个入口不要混用；`currency == "USD"` 用 `$`。`demo/panel-demo-e.html` 的示例金额与 `docs/SPEC.md` §12.5 已同步这条规则；A–D 四版是被否决的历史对照，里面的 `¥10` / `¥100` 保持原样，**不要拿它们当格式依据**。
  - token：`1.33M` / `284K`（M 最多两位、K 最多一位、去掉尾零）。
  - 命中率：一位小数、无尾零；没有 prompt token 的行显示 `cache --`（不着色）。
  - 模型占比条是 **token 份额**（本模型本月 token ÷ 本组最大），不是金额份额。
  - 过滤全零占位与「按本月消费取前 4」在 `fetch_diag` 结尾统一做一次：IPC 载荷、`deepseek-updated` 事件与 `deepseek-cache.json` 里都只留面板要画的行，前端不再二次筛选，两份逻辑不可能漂移。诊断行（`--verbose` 的逐模型一行）在那之前打印，所以仍能看到全部模型。
- 错误码 → 文案（配合 §5.3：数字保留、状态行说明）：
  - 余额：`no-api-key` → `Not configured` + 说明行 `API key not set`；`key-invalid` → `Key invalid`；`rate-limited` → `Rate limited`；其余（含 `http-N` / `network-error` / `parse-error`）→ `Fetch failed`。
  - 用量（整块换成一行提示）：`no-user-token` → `Usage token not configured`（不写「仅余额可用」——余额块自己会说，两个凭证都缺时那句还会自相矛盾）；`token-expired` → `Usage token expired — sign in again to sync`；`rate-limited` → `Usage was rate limited — retrying in 30s`；其余 → `Usage fetch failed — retrying in 30s`。本月没有用量（模型列表为空）→ `No usage data this month`，不着红。
  - header 时间戳跟当前 tab：`no-api-key` → `Not configured`；其他错误 → `Fetch failed`；正常 → `Updated HH:MM`，非当天 → `Updated M/D HH:MM`（DeepSeek 会跨会话复用缓存，只写时刻会把昨天读成今天）。

### 5.3 轮询与失败语义

- 启动 2s 后首次拉取；正常间隔 1/5/10/30 分钟（默认 5）。
- 失败：保留上次成功数据 + 状态行提示，30s 后快速重试；成功后回正常周期。失败文案统一为 **`Fetch failed`**（面板 header 红字）与 **` (fetch failed)`**（托盘 tooltip）—— 原来的 `Update failed` 容易被误读成 CLI 版本检查失败，而那是一条独立的路。
- 托盘悬停预取两个 provider，10s 节流。
- 两个 provider 各自独立 poller，共用上述语义。

## 6. DeepSeek 漂移加固

模型名本来就是数据（`usage[].model`），新模型只会显示原始 ID，官方另有公开端点可提供展示名称；接口路径/参数则是硬编码常量。四层加固：

1. **零成本 + 官方命名元数据**：显示名按四段回落 —— 官方 `GET https://api.deepseek.com/models` 的 `name`（Bearer 同一 API Key，按 id 精确匹配）→ 静态表（已知 id，离线兜底）→ 规则派生（去 `deepseek-` 前缀、`v4-pro` → `V4 Pro`、剥离 `-0`/日期后缀）→ 原始 id。
   **优先级于 2026-11-14 调整（用户决定）**：官方名压过静态表，且必须跟随厂商上游的实际命名变动 —— 上游改名后最长 24 小时（缓存 TTL）即反映到面板，不需要发新版本；静态表退化为「拿不到网络时的兜底」。派生只对 `deepseek-*` 形式的 id 生效，含空白的组合行（如 `A & B`）与外部 id 原样显示，不做任何改写。
   刷新与缓存：启动后首次校验抓一次；出现未知 id 时补抓一次（每次启动最多一次）；结果写 `config_dir()/deepseek-models.json`，TTL 24 小时；无 API Key、请求失败或 `name` 缺失都静默回落到下一段。只取 `id` 与 `name`，其余字段（`context_window` / `effort` / `api_capabilities`）与额度面板无关，不解析、不落盘。
   注意：该端点属于 `api.deepseek.com`，与用量所在的 `platform.deepseek.com` 内部接口无关，**不解决路径/参数漂移**；用量口径来自平台控制台聚合，可能出现组合行（`A & B`）或已下线模型，因此**永不因为某个 id 不在 `/models` 里就丢弃它的用量**。
   诊断：`--test-deepseek --verbose` 打印所用端点 + 响应顶层键 + 每模型聚合 + `/models` 命中情况。
2. **候选链**：常量之外准备候选（`/api/v0/usage` → `/api/v1/usage` …），遇 404/410 逐个试，成功后写入 `config_dir()/deepseek-endpoints.json`，之后优先用记住的。
3. **从登录窗学习**：WebView2 hook 已经能看到控制台发出的每个请求；除 `Authorization` 外，把该请求的 `{url, 方法, 非敏感头}` 记成模板（token 仍只进 `deepseek-credentials.json`），路径/参数名/`x-*` 头都能自愈。
4. **兜底提示**：HTTP 200 但每个模型聚合全为 0、而响应体明显有数据时，面板显示"响应格式可能已变化，点此重新同步"。

边界：若上游改为服务端签名/设备指纹/验证码，静态重放无效，只能靠同进程 WebView2 的真实会话，或改抓页面数据。

## 7. 平台行为约定（不可回归）

- **单实例**：命名互斥锁 `QuotaTray.SingleInstance`（新名字，迁移期可与两个旧版并存）。
- **DPI**：窗口尺寸用 `LogicalSize`、位置用 `PhysicalPosition`（tao 在 `WM_DPICHANGED` 上会二次缩放物理尺寸）。
- **Win11 圆角**：每个窗口 `DWMWA_WINDOW_CORNER_PREFERENCE = DONOTROUND`，圆角由 CSS 自绘。
- **便携模式**：exe 旁存在 `portable.dat` 时配置写 exe 目录，否则 `%APPDATA%\QuotaTray`。
- **开机自启**：HKCU Run，仅用户域，永不请求管理员权限。
- **面板交互**：托盘左键开合；失焦自动隐藏（settings/skills 打开时抑制）；隐藏后 300ms 内的托盘点击忽略（同一次点击造成的失焦）。**关掉设置窗要补一次收起判定**：抑制期间那次 blur 已经被丢掉，关窗也不会再产生新的 blur，所以 `close_settings` 之后 150ms 检查「面板可见且未获焦点」则收起（150ms 是留给 Windows 把焦点交还面板的时间）。
- **WebView2 recovery**：沿用 kimi 的策略——`ProcessFailed` 时就地重建窗口，不重启进程，`ExitRequested` 时 `prevent_exit`；重建单飞 + 1/2/5s 退避 + 30s 内 3 次封顶；诊断写 `webview.log`，永不写 token。
- **错误呈现**：所有 IO / 注册表 / 进程 / HTTP 失败都静默吞掉，只通过界面文字与状态字段暴露，**不弹对话框**。
- **settings.json**：`{ Theme, RefreshMinutes, AutoStart, ActiveTab }`，PascalCase，`serde(default)` 容忍缺字段；`ActiveTab` 缺省回 `kimi`，写入时也只接受 `kimi` / `deepseek` 两个值（其他一律当 `kimi`）。

## 8. 阶段计划与验收

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0（已完成）** | 四版静态 demo + 设计定稿 + 本文件 | `demo/compare.html` 可切主题/provider/状态；`heights.py` 出高度表；`check_contrast.py` 全通过 |
| **P1（已完成）** | 仓库化、拷入 `kimi-planbar-tray/rust/` 基线、改名 Quota Tray、拷入 SPEC/LICENSE | `cargo build` + `npm run build` + `tsc --noEmit` 通过；`--test-fetch` / `--test-ui` 通过；旧 UI 能跑 |
| **P2（已完成 2026-11-14）** | 面板 UI 重写（E 版 token）+ tab + 自适应高度。**已完成**：四个窗口统一字体/结构与 D 的骨架、强调色换成静紫（E 版）、面板与设置窗内容自适应、kimi-cu 端到端验过；分段 tab（E 的紫 ring）+ DeepSeek 面板（真实数据、阈值、错误文案、`ActiveTab` 持久化） | 连点 tab 30 次不跳位、底部贴边不变；`--test-ui` 通过；截图与 `panel-demo-e.html` 对照一致 |
| **P3（已完成 2026-11-14）** | DeepSeek 后端移植 + 双 poller + fixtures 解析测试 | ✅ `cargo test` 18 passed；✅ 同机对比旧版 `--test-fetch` 逐字段一致（仅展示名按 §6.1 取官方名）；✅ `--test-deepseek` 通过；旧版 `--test-fixtures` ALL PASS |
| **P4** | 设置窗（通用 / Kimi 只读 / DeepSeek 凭证）+ 托盘合并 tooltip + 登录捕获窗 | 设置落盘后重启生效；真账号走通捕获 → 验证 → 保存 → 面板出数；`--test-login` 通过 |
| **P5** | 打包、端到端验证、README、独立 code review | `npx tauri build` 出单 exe；`--test-fetch` / `--test-deepseek` / `--test-ui` / `--test-login` 全绿；端到端脚本通过；审查报告无 Blocker |

**顺序调整（2026-11-14 决定）**：P2 剩余的分段 tab 移到 P3 之后。理由是先让 DeepSeek 有真实数据，tab 一次做对，不做空态占位面板。tab 于同日随后落地，验收仍是「连点 30 次不跳位、底边与右边缘不动」。

## 9. 风险与未决

- **底部锚定改高**：`set_size` + `set_position` 在可见窗口上可能闪一帧；缓解：一次命令内完成、面板未显示时先定尺寸。
- **WebView2 COM 互操作**：Tauri 的 `with_webview` + `webview2-com` 版本必须与 wry 一致（当前 0.39）；登录窗回调里只做轻量工作，验证/落盘放后台线程。
- **私有接口漂移**：见 §6；失败必须表现为可理解提示 + 可重试。
- **内存**：WebView2 常驻约 300MB，与现版 kimi 一致，不做优化承诺。
- **未决**：
  - ~~分段控件的活动格怎么表达选中~~（**已定，2026-11-14**：`--seg-ring` 取强调色，面板 tab 与设置窗共用。填充差 1.14 / 1.33 不够 1.4.11 的 3:1；紫 ring 对轨道 4.46（浅）/ 4.82（深），对活动块 5.08 / 3.63，四端全过。起因与前后对比见 **`demo/seg-ring-problem.html`**，该页已改为「记录」性质）。
  - 托盘图标是否换新（当前沿用 kimi 的 K 标）。
  - ~~skills 窗口是否也随 D 版换肤~~（已定：随 D 版换肤，2026-11-13 落地；E 起强调色变化同样适用于四个窗口）。
  - **设置窗开关边界**：`--hair` 只给到 1.34:1（轨道描边），低于 WCAG 1.4.11 的 3:1。已改为 `--ink-2` 描边（5.67:1 亮 / 6.26:1 暗），开启态是 `--ink-1` 实心轨道，两态都能分辨；与 tab ring 同一取舍：状态不靠填色差，靠形状与位置。

## 10. 设计审阅工具（`demo/`）

```bash
bash demo/shot.sh demo/panel-demo-e.html out.png "bare=1&theme=dark&tab=ds" 560 900 2
PYTHONUTF8=1 python demo/crop_panel.py out.png out-crop.png 424 2
PYTHONUTF8=1 python demo/heights.py        # 四版 × 两 tab × 五状态的高度表
PYTHONUTF8=1 python demo/check_contrast.py # token 对的 WCAG 比值
```

`shot.sh` 自动探测 Chrome → Edge（Windows 浏览器不能写 MSYS 路径，脚本内部用 `cygpath` 转换）。临时截图写 `.shots/`（已 gitignore），用完即删。

## 11. 命名与路径

| 项 | 值 |
|---|---|
| 产品名 | `Quota Tray` |
| identifier | `com.shawnqi.quotatray` |
| crate / 前端包名 | `quota-tray` |
| exe | `quota-tray.exe` |
| 配置目录 | `%APPDATA%\QuotaTray`（便携模式：exe 同级） |
| 诊断覆盖变量 | `QUOTA_TRAY_APP_DATA_DIR` |
| DeepSeek 凭证 | `config_dir()/deepseek-credentials.json`（值 `dpapi:` 加密，键名明文） |
| DeepSeek 缓存 / 展示名 / 端点 | `config_dir()/deepseek-{cache,models,endpoints}.json` |
| 日志 | `config_dir()/webview.log` |
