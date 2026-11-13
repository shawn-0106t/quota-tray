# PLAN — Quota Tray

> 状态：**设计定稿，代码待落地**（Phase 1 起）。
> 视觉基准：`demo/panel-demo-d.html`（唯一准绳；`demo/compare.html` 保留 A/B/C 三版仅作对照，不再演进）。
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
| 视觉方向 | D 版：C 的结构 + A 的字体 + A 的配色（收敛到两色） | 结构上 C 的两行 header + 无框读数区最清爽；字体上 A 的全 sans 比 mono 更耐看；配色上 A 的暖纸底 + alpha 墨阶，但砍掉多余色相 |
| 字体 | 全系统字体，零下载；**不用 Inter** | `kimi-planbar-tray` 本身就是 `'Segoe UI', system-ui, sans-serif` 一条声明；Windows 小字号下 Segoe UI 的 hinting 最优，Inter 需打包字体且与 CJK 混排基线不一致 |
| 数字字体 | sans + `tabular-nums`，**不使用 mono** | 等宽数字的"代码感"与托盘工具的定位不符；`tabular-nums` 已能保证刷新不跳宽 |
| 强调色 | 单一 Kimi 蓝（`#1783ff` / dark `#1a88ff`），两个 tab 共用 | 应用本身是 Kimi 的延伸；DeepSeek 品牌蓝、成功绿、警告琥珀全部取消，红只留给真实故障 |
| tab 控件 | 灰底轨道 + 等宽单元格 + 活动块用 1px 内描边 ring（**不用投影**） | 去投影后更平面；ring 比阴影更省视觉噪音 |
| 合并版仓库 | 新建独立 repo，两个原仓库不动 | 原仓库有各自的 CI/发布线，合并进去会污染语义 |
| 产品名 | **Quota Tray**；`com.shawnqi.quotatray`；`quota-tray.exe` | — |
| 版本 | 0.1.0 起 | — |
| DeepSeek 登录捕获 | 进 v1 | 与现版功能对齐；代价是 WebView2 COM 互操作是本项目风险最高的一块 |

## 3. 设计定稿（D 版）

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

DeepSeek tab 用同一套骨架：`BALANCE` + `Available` / `¥66.50` / `Granted … Topped up …` → 每模型一行（名称 + 金额 / 占比条 / `1.33M tok · cache 80%` + `today ¥0.30`）→ 合计行。

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
| `--accent` | `#1783ff` | `#1a88ff` | 品牌蓝（选中态之外的强调基准） |
| `--accent-bar` | `#0d6fe8` | `#1a88ff` | 进度条填充、focus ring（对轨道 ≥3:1） |
| `--accent-ink` | `#0a5fc0` | `#6fb4ff` | chip 文字（对 tint ≥4.5:1） |
| `--tint` | `#1783ff1a` | `#1a88ff26` | chip 底 |
| `--danger` | `#b42318` | `#ff6b6b` | 故障红 |
| `--seg-track` | `#f1f0ec` | `#262624` | 分段控件轨道；footer hover 底 |
| `--seg-on` | `#ffffff` | `#3a3a37` | 活动格底 |
| `--seg-ring` | `#0000001f` | `#ffffff1a` | 活动格内描边 |
| 阴影 | `0 12px 40px #00000026` | `0 12px 40px #00000073` | 面板悬浮阴影（case 的 menu 档） |

### 3.3 排版

- 字族：`'Segoe UI', system-ui, -apple-system, 'PingFang SC', 'Microsoft YaHei UI', 'Microsoft YaHei', sans-serif`（与 `kimi-planbar-tray` 的 rust 前端同一条）。
- 全局 `font-variant-numeric: tabular-nums`。
- 层级：字标 13/500 · 时间戳 11/400 · tab 12.5（未选 400 / 选中 500）· 微标签 10/500 大写 `letter-spacing: .08em` · 主数字 30/600 · 单位 15/500 · 行标签 12/400 · 行值 13/600 · 说明 10.5/400 · footer 10.5/500 · chip 9.5/500。
- 行值里的纯文字（`No data` / `Not activated`）用 13/500，不继承 600。

### 3.4 组件规格

| 组件 | 规格 |
|---|---|
| 分段控件 | margin `10 0 12`，padding 3，轨道 radius 12；格高 30、radius 8、`flex: 1`；活动格 `--seg-on` + `inset 0 0 0 1px --seg-ring` |
| 读数列 | `grid-template-columns: 1fr 1fr; gap: 24px`，列内 padding `10 0 11` |
| 进度条 | 高 4、radius 2、轨道 `--hair-soft`、填充 `--accent-bar`、`margin-top: 9`；模型占比条高 3、`margin-top: 8` |
| 分行 | `border-top: 1px --hair`，padding `9 0`；**最后一个子块** `padding-bottom: 10`（父容器不再加底部内边距，否则该行会光学偏上） |
| 模型行 | 名称单行省略（`min-width: 0` + `text-overflow: ellipsis`、`white-space: nowrap`），金额固定右对齐、不参与省略 —— 官方展示名可能比手写表长，不能顶掉金额 |
| chip | radius 9999，padding `2 8`，`--tint` 底 + `--accent-ink` 字 |
| footer | `grid 4`，`gap 2`，padding 5，按钮高 42、radius 8、图标 16、hover 底 `--seg-track` |
| 开关 | 轨道 38×20、radius 9999、1px `--ink-2` 描边；钮 14、inset 2；开启态轨道 `--ink-1`、钮 `--paper`、位移 18px；150ms 过渡；整行是点击目标；**单色，不占蓝**（蓝只给用量） |
| 焦点 | `outline: 2px solid --accent-bar; outline-offset: 2px` |

### 3.5 颜色语义

- **蓝 = 用量**（进度条、chip）。不做"高用量变色"。
- **红 = 故障**（余额不足、token 过期、Key 失效、更新失败、解析失败、命中率 <60%）。
- **墨 = 一切正常**。`Available` 不着色，用量百分比不着色。

### 3.6 动效

- 面板开场：opacity 160ms linear + translateY 16→0 220ms `cubic-bezier(.215,.61,.355,1)`；收场 130/160ms，结束后回调后端真正隐藏（沿用 kimi 的 `finish_hide` 语义）。
- tab 切换：内容 120ms 交叉淡入，窗口高度同帧变更（底部锚定）。
- 全局 `prefers-reduced-motion` 关闭过渡。

## 4. 窗口与信息架构

| 窗口 | 尺寸 | 定位 | 说明 |
|---|---|---|---|
| `main` | 424 × 自适应（典型 330–545 + 56 留白） | 工作区右下角 12px | 无边框、透明、置顶、不占任务栏、启动时隐藏 |
| `settings` | 404 × 自适应（内容上报，上限工作区 90%；当前约 347） | 屏幕居中 | 分区：通用 / Kimi 只读信息 / DeepSeek 凭证 |
| `skills` | 404 × 520 | 屏幕居中 | Kimi 专属、只读、零后台开销 |
| `menu` | 188 × N | 托盘光标处 | Open / Refresh / Settings / Skills / Exit |
| `login`（DeepSeek） | ~900 × 680 | 居中 | 临时窗，不进 recovery 集合 |

面板高度由前端测量内容后上报（`panel_height`），后端用 `LogicalSize` 改高 + `PhysicalPosition` 把窗口**右下角钉回原位**，并按工作区高度夹取；`show_panel` 先按当前 tab 定尺寸再显示，避免可见跳变。tab 选择持久化到 settings.json 的 `ActiveTab`。

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
- 凭证：`DEEPSEEK_API_KEY` / `DEEPSEEK_USER_TOKEN` 环境变量优先 → `~/.deepseek-planbar/credentials.json`（**沿用旧路径，老用户零重填**）；缓存 `~/.deepseek-planbar/cache.json`（原子写，保留上次成功值）。
- 错误码：`key-invalid` / `token-expired` / `rate-limited` / `server-error` / `http-N` / `network-error` / `parse-error`；余额与用量两类错误互相独立。
- 解析：`data.biz_data` 既可能是对象也可能是数组（取 `[0]`）；`total[]` 按模型聚合，`REQUEST` 计请求不计 token，`PROMPT_CACHE_HIT/MISS_TOKEN` 算命中率；`days[]` 只取本地今天；金额保 double 精度、token 取整；过滤全零占位模型；按本月消费降序取前 4。
- 阈值：命中率 <60% 标红（不含琥珀档）；余额 < ¥10 或 `is_available=false` 标红。

### 5.3 轮询与失败语义

- 启动 2s 后首次拉取；正常间隔 1/5/10/30 分钟（默认 5）。
- 失败：保留上次成功数据 + 状态行提示，30s 后快速重试；成功后回正常周期。
- 托盘悬停预取两个 provider，10s 节流。
- 两个 provider 各自独立 poller，共用上述语义。

## 6. DeepSeek 漂移加固

模型名本来就是数据（`usage[].model`），新模型只会显示原始 ID，官方另有公开端点可提供展示名称；接口路径/参数则是硬编码常量。四层加固：

1. **零成本 + 官方命名元数据**：显示名按四段回落 —— 静态表（已知 id，离线兜底）→ 官方 `GET https://api.deepseek.com/models` 的 `name`（Bearer 同一 API Key，按 id 精确匹配）→ 规则派生（去 `deepseek-` 前缀、`v4-pro` → `V4 Pro`、剥离 `-0`/日期后缀）→ 原始 id。
   刷新与缓存：启动后首次校验抓一次；出现未知 id 时补抓一次（每次启动最多一次）；结果写 `~/.deepseek-planbar/models.json`，TTL 24 小时；无 API Key、请求失败或 `name` 缺失都静默回落到下一段。只取 `id` 与 `name`，其余字段（`context_window` / `effort` / `api_capabilities`）与额度面板无关，不解析、不落盘。
   注意：该端点属于 `api.deepseek.com`，与用量所在的 `platform.deepseek.com` 内部接口无关，**不解决路径/参数漂移**；用量口径来自平台控制台聚合，可能出现组合行（`A & B`）或已下线模型，因此**永不因为某个 id 不在 `/models` 里就丢弃它的用量**。
   诊断：`--test-deepseek --verbose` 打印所用端点 + 响应顶层键 + 每模型聚合 + `/models` 命中情况。
2. **候选链**：常量之外准备候选（`/api/v0/usage` → `/api/v1/usage` …），遇 404/410 逐个试，成功后写入 `~/.deepseek-planbar/endpoints.json`，之后优先用记住的。
3. **从登录窗学习**：WebView2 hook 已经能看到控制台发出的每个请求；除 `Authorization` 外，把该请求的 `{url, 方法, 非敏感头}` 记成模板（token 仍只进 `credentials.json`），路径/参数名/`x-*` 头都能自愈。
4. **兜底提示**：HTTP 200 但每个模型聚合全为 0、而响应体明显有数据时，面板显示"响应格式可能已变化，点此重新同步"。

边界：若上游改为服务端签名/设备指纹/验证码，静态重放无效，只能靠同进程 WebView2 的真实会话，或改抓页面数据。

## 7. 平台行为约定（不可回归）

- **单实例**：命名互斥锁 `QuotaTray.SingleInstance`（新名字，迁移期可与两个旧版并存）。
- **DPI**：窗口尺寸用 `LogicalSize`、位置用 `PhysicalPosition`（tao 在 `WM_DPICHANGED` 上会二次缩放物理尺寸）。
- **Win11 圆角**：每个窗口 `DWMWA_WINDOW_CORNER_PREFERENCE = DONOTROUND`，圆角由 CSS 自绘。
- **便携模式**：exe 旁存在 `portable.dat` 时配置写 exe 目录，否则 `%APPDATA%\QuotaTray`。
- **开机自启**：HKCU Run，仅用户域，永不请求管理员权限。
- **面板交互**：托盘左键开合；失焦自动隐藏（settings/skills 打开时抑制）；隐藏后 300ms 内的托盘点击忽略（同一次点击造成的失焦）。
- **WebView2 recovery**：沿用 kimi 的策略——`ProcessFailed` 时就地重建窗口，不重启进程，`ExitRequested` 时 `prevent_exit`；重建单飞 + 1/2/5s 退避 + 30s 内 3 次封顶；诊断写 `webview.log`，永不写 token。
- **错误呈现**：所有 IO / 注册表 / 进程 / HTTP 失败都静默吞掉，只通过界面文字与状态字段暴露，**不弹对话框**。
- **settings.json**：`{ Theme, RefreshMinutes, AutoStart, ActiveTab }`，PascalCase，`serde(default)` 容忍缺字段；`ActiveTab` 缺省回 `kimi`。

## 8. 阶段计划与验收

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0（已完成）** | 四版静态 demo + 设计定稿 + 本文件 | `demo/compare.html` 可切主题/provider/状态；`heights.py` 出高度表；`check_contrast.py` 全通过 |
| **P1（已完成）** | 仓库化、拷入 `kimi-planbar-tray/rust/` 基线、改名 Quota Tray、拷入 SPEC/LICENSE | `cargo build` + `npm run build` + `tsc --noEmit` 通过；`--test-fetch` / `--test-ui` 通过；旧 UI 能跑 |
| **P2（进行中）** | 面板 UI 重写（D 版 token）+ tab + 自适应高度。**已完成**：四个窗口统一 D 版视觉（含设置/技能/托盘菜单）、面板与设置窗内容自适应 | 连点 tab 30 次无跳位、底部贴边不变；`--test-ui` 通过；截图与 `panel-demo-d.html` 对照一致。**剩余**：分段 tab + DeepSeek 空态面板（依赖 P3 数据） |
| **P3** | DeepSeek 后端移植 + 双 poller + fixtures 解析测试 | `cargo test` 通过；同机对比旧版 `--test-fetch` 逐字段一致；`--test-deepseek` 通过 |
| **P4** | 设置窗（通用 / Kimi 只读 / DeepSeek 凭证）+ 托盘合并 tooltip + 登录捕获窗 | 设置落盘后重启生效；真账号走通捕获 → 验证 → 保存 → 面板出数；`--test-login` 通过 |
| **P5** | 打包、端到端验证、README、独立 code review | `npx tauri build` 出单 exe；`--test-fetch` / `--test-deepseek` / `--test-ui` / `--test-login` 全绿；端到端脚本通过；审查报告无 Blocker |

## 9. 风险与未决

- **底部锚定改高**：`set_size` + `set_position` 在可见窗口上可能闪一帧；缓解：一次命令内完成、面板未显示时先定尺寸。
- **WebView2 COM 互操作**：Tauri 的 `with_webview` + `webview2-com` 版本必须与 wry 一致（当前 0.39）；登录窗回调里只做轻量工作，验证/落盘放后台线程。
- **私有接口漂移**：见 §6；失败必须表现为可理解提示 + 可重试。
- **内存**：WebView2 常驻约 300MB，与现版 kimi 一致，不做优化承诺。
- **未决**：
  - tab 活动块的 ring 用中性灰时，对轨道/活动块只有约 1.2–1.3:1，低于 WCAG 1.4.11 对状态标识的 3:1；当前靠「填充差 + 字重 + `aria-selected`」表达状态，键盘焦点另有 2px 蓝 outline（3.8:1）兜底。可选改成强调蓝 ring（对轨道 3.8:1，代价是标题区多一个蓝色元素）——待定。
  - 托盘图标是否换新（当前沿用 kimi 的 K 标）。
  - ~~skills 窗口是否也随 D 版换肤~~（已定：随 D 版，2026-11-13 落地）。
  - **设置窗开关边界**：`--hair` 只给到 1.34:1（轨道描边），低于 WCAG 1.4.11 的 3:1。已改为 `--ink-2` 描边（5.67:1 亮 / 6.26:1 暗），开启态是 `--ink-1` 实心轨道，两态都能分辨；与 tab ring 同一取舍：状态不靠填色差，靠形状与位置。

## 10. 设计审阅工具（`demo/`）

```bash
bash demo/shot.sh demo/panel-demo-d.html out.png "bare=1&theme=dark&tab=ds" 560 900 2
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
| DeepSeek 凭证/缓存 | `~/.deepseek-planbar/{credentials,cache}.json`（沿用） |
| 日志 | `config_dir()/webview.log` |
