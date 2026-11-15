# Slint spike — 视觉上限实测

> 交互（P0）的实测另见 `spike/p0/REPORT.md`；本文只覆盖静态视觉。

> 结论先行：**布局可以做到 0 px 偏差，文字不可能逐像素一致。** 424 px 面板的 18 个（Kimi）/ 22 个（DeepSeek）文本与分隔线位置和 Chromium 完全对齐（Δy = 0 设备像素），整窗仍有 2.5%–3.6% 的像素差（>8/255），全部来自字形栅格化与字体 shaping。**两个 renderer（femtovg / Skia）都试过：换 renderer 不改变文字差距——同一串的墨迹宽度一分不差——只改变字形栅格化与阴影。** 要不要接受，取决于「一致」的标准是「肉眼一致」还是「截图能对上」。

## 怎么量的

两边是**同一份 DOM、同一套 CSS、同一组数据**，在**同一台机器、同一 scale=2** 下渲染：

- 参考侧：`spike/slint-demo/ref/build_ref.py` 把仓库里的 `index.html` + `src/theme.css` + `src/main.css` 内联成一页，用与 `demo/build_ui_demo.py` 相同的示例数据（Kimi 12/31、cache 99.5/99.7、DeepSeek ¥45.44 + 4 个模型），headless Edge 截图。
- Slint 侧：`slint-demo` 用**从浏览器 `getBoundingClientRect()` 取出的坐标**逐元素绝对定位（不是手算的 CSS 盒子模型），`Window::take_snapshot()` 出图。
- 对比：`demo/slint-compare.py` 生成并排图、50% 叠加图、×8 差值热力图，并按行统计文本带（band）位置。

复现：

```bash
PYTHONUTF8=1 python spike/slint-demo/ref/build_ref.py          # 参考图 + rect 基准
cd spike/slint-demo && cargo build --release
for t in light dark; do for tab in kimi ds; do \
  ./target/release/slint-demo.exe --theme $t --tab $tab --renderer wgpu --scale 2 \
    --out ../../.shots/slint-demo/slint-$t-$tab.png; done; done
cd ../.. && PYTHONUTF8=1 python demo/slint-compare.py
```

看结果：`.shots/slint-demo/compare.html`（HTML / femtovg / Skia 三栏并排、各自的 50% 叠加图、差值热力图、每条文本带的 Δy 与探针表）。

```bash
# Skia 组（--blur 18 是 Skia 上与 CSS 等价的取值，见下）
for t in light dark; do for tab in kimi ds; do \
  ./target/release/slint-demo.exe --theme $t --tab $tab --renderer skia --scale 2 --blur 18 \
    --out ../../.shots/slint-demo/skia-$t-$tab.png; done; done
```

## 数字

| case | 窗口（CSS） | 文本带 | 带位置偏差 | 像素差 >8/255 | 平均 \|Δ\| |
|---|---|---|---|---|---|
| light / Kimi | 424 × 560 | 18 | **0**（末带 +1.5 px 为圆角） | 2.53% | 2.24 |
| light / DeepSeek | 424 × 601 | 22 | **0**（同上 +1.5 px） | 3.47% | 3.59 |
| dark / Kimi | 424 × 560 | 18 | **0** | 2.66% | 2.08 |
| dark / DeepSeek | 424 × 601 | 22 | **0** | 3.55% | 3.21 |

对照：坐标没对齐前（参考图 rect 用 DPR=1 取、截图用 DPR=2）是 6.8% / 5.8% / 6.8% / 5.8%，Kimi 还出现 -1 → -5 的累积漂移。**把 rect 的采样 DPR 对齐后，布局偏差直接归零**——这是这次最值钱的一条经验：Chrome 在 DPR=2 下对 `line-height: normal` 的取整与 DPR=1 不同，每块文本差 0.5–1 px，叠起来就是 3 px。

### 两个 renderer 的对照

| case | renderer | 像素差 >8 | 平均 \|Δ\| | 末带 Δy | brand 墨迹宽差 |
|---|---|---|---|---|---|
| light/Kimi | femtovg | 2.53% | 2.24 | 0 | -4 |
| light/Kimi | Skia | 2.52% | 2.22 | 0 | -4 |
| light/DeepSeek | femtovg | 3.47% | 3.59 | 0 | -4（model4 -11） |
| light/DeepSeek | Skia | 3.43% | 3.56 | 0 | -4（model4 -11） |
| dark/Kimi | femtovg | 2.66% | 2.08 | **3** | -5 |
| dark/Kimi | Skia | 2.51% | 1.94 | **0** | -5 |
| dark/DeepSeek | femtovg | 3.55% | 3.21 | **3** | -5（model4 -12） |
| dark/DeepSeek | Skia | 3.44% | 3.08 | **0** | -5（model4 -12） |

墨迹宽度（探针：`span.brand`、"DeepSeek-V4.1-Reasoner-Pro-Max-Extended"、首行金额）两个 renderer **完全相同**。也就是说：run 的 advance 来自 parley/fontique 的 shaping，两个 renderer 都只是照着画；换 renderer 不可能修掉字距差。Skia 唯一可量测的胜点是阴影（下节）和暗色下末带 Δy 归零。

## 完全对得上的部分

- **所有块的 y 坐标**：18 / 22 条文本带的顶端与参考完全一致（0 设备像素）。
- **所有 1 px 分隔线**：cache / decode / extra / version / footer 的上边线，以及模型行之间的横线，逐条同 y。
- **进度条**：track 与 fill 的 x/y/宽/高一致（12% → 18.75 px，99.5% → 156.2 px）。
- **颜色**：两套 token 逐值一致（`#fbfaf9` / `#181817` / `#8c74b1` / `#7d61a6` / `#8c74b12e` …），暗色下 `cache 58.4%` 的红、`Update` chip 的紫底紫字都对。
- **几何**：卡片 368 px 宽、12 px 圆角、1 px 发丝边；分段控件 338 × 36、格 165 × 30 / 半径 8 / 激活格 1 px 紫环；footer 4 × 87.5 × 42、图标 16 px。
- **透明窗**：`--stage none` 的快照里 13.8% 像素 alpha = 0（28 px 留白之外）、8.2% 半透明（阴影衰减带）、卡片内部全不透明 —— 逐像素 alpha 窗口是真的，不是拿底色冒充的。
- **右对齐金额**：`¥28.50` / `¥0.88` 的右边缘与参考差 ≤0.5 设备像素。

## 对不上的部分

1. **字形栅格化（无解）**。Chromium 走 DirectWrite，Slint 走 parley/fontique + femtovg，同一个 `Segoe UI` 的字形轮廓与抗锯齿不同：50% 叠加图里每个词内部都「重影」，即使词的起止边缘对齐。字重 500 尤其明显（Segoe UI 没有 500 这一档，两边各自取的近似不同）。
2. **整串文字的宽度**：实测同串同字号，"Quota Tray" Slint 窄 2.5 CSS px，两个模型名各宽 0.5 px，右对齐金额一致。也就是说**不是整体缩放**，而是逐字 advance/kerning 的零散差异；居中/右对齐的串会左右各漂 1–2 px，`12` 与 `%` 这种相邻元素（unit 的 x 绑在数字宽度之后）会整体挪 1–2 px。
3. **阴影的尾巴**：CSS `box-shadow: 0 8px 18px` 与 Slint `drop-shadow-*` 不是同一个单位，而且**两个 renderer 的映射不同**：
   - femtovg：扫 18/30/42/54 后最佳是 **42**（采样带平均差 0.76 灰阶），卡片上方仍少伸 ~9 设备像素（4.5 CSS px）；
   - Skia：最佳是 **18，与 CSS 同值**（平均差 0.52、最大 1 灰阶），阴影范围与 CSS 几乎重合（暗色下末带 Δy 从 3 降到 0）。
   选 Skia 的话 `--shadow` token 可以直接沿用 CSS 的 `0 8px 18px`，不用重标定；选 femtovg 就要把 blur 改成 42。
4. **`font-variant-numeric: tabular-nums` 没有对应开关**。Segoe UI 的数字本身近似等宽，这次没有可见后果；但换成别的字体就会跳宽。

## 构建与引擎事实

- **两个 renderer 都能跑，成本差一档**：femtovg-wgpu 纯 Rust，增量构建 ~45 s；Skia 走 `skia-bindings` 预编译包（修正 hosts 后直连 GitHub 下载，构建 93 s），exe 从 14.6 MB 涨到 **21.1 MB**，依赖树 723 → 751 个 crate。Skia 初始化会打印 `Failed to initialize Skia wgpu surface … Falling back to the next surface candidate`（它先试 wgpu/Vulkan，不支持就回落到自己的 GL/D3D surface），出图正常，这条警告可忽略。
- **femtovg-wgpu 与 femtovg(GL) 出图逐像素相同**（平均差 0.00，最大 1/255）；femtovg 与 Skia 之间差 1.3%–1.8% 的像素（纯字形栅格化）。
- **产物**：femtovg 版 14.6 MB / 723 crate，Skia 版 21.1 MB / 751 crate（Tauri 版是 7.2 MB / 498 个）。Slint 不等于更轻的依赖树，除非只留 GL renderer。
- 冷编译约 6 分钟（wgpu + Slint），之后增量 ~45 s。

## 这次没验证的

只测了**静态视觉 + 控件外观**。以下都还没有证据，属于全量阶段的真实风险：切 tab 的 167/200 ms 动效与「窗口先放大、卡片底边不动」、失焦自动隐藏、托盘三行 tooltip 与左右键、`--login-host` 的 webview 捕获、三档 DPI、键盘与 IME、滚动条。

## 两条路的判断

- **如果「一致」= 截图能对上、可做像素级回归**：只有保留 WebView2 的 `wry` 换壳能做到（同一份 `dist/`、同一个 Chromium，构造上逐像素一致）。
- **如果「一致」= 看上去一样、行为一样**：Slint 可行，且应当选 **Skia**——只有它能把阴影与 CSS 1:1 对上（blur 18），暗色下末带 Δy 也归零；femtovg 要把 blur 改成 42 才接近，且卡上方仍短 4.5 px。代价是这套像素门禁（`demo/heights.py`、`make_readme_shots.py` 的裁切断言、截图字节一致）要重定基线，改成「文本带 Δy = 0 + 像素差阈值」，并接受字形重影永远存在（换 renderer 修不掉，因为差距在 shaping 层）。

推荐：先看 `.shots/slint-demo/compare.html` 里的 50% 叠加图。如果 1:1 观看时那层重影可以接受，Slint 值得做；如果要求「叠图也对得上」，直接走 `wry`，不要在这个方向上再投入。
