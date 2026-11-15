import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import './theme.css';
import './main.css';
import {
  AppStateDto,
  DeepseekModelUsage,
  DeepseekResult,
  DeepseekUsage,
  ExtraInfo,
  KimiCacheWindow,
  KimiUsage,
  ProviderTab,
  QuotaResult,
  QuotaSegment,
  SHADOW_ROOM,
  UpdateStatus,
  clampPercent,
  fmtCount,
  fmtHitRate,
  fmtMoney,
  fmtPct,
  fmtTokens,
  fmtUpdated,
  fmtYuan,
  formatReset,
  initTheme,
  isLowBalance,
  isLowCacheHit,
} from './common';

function el<T extends HTMLElement>(id: string): T {
  return document.getElementById(id) as T;
}

// ---- Module state: both providers stay rendered, the tab decides what shows ----

let tab: ProviderTab = 'kimi';
let quota: QuotaResult | null = null;
let deepseek: DeepseekResult | null = null;
let kimiUsage: KimiUsage | null = null;

// ---- Rendering: Kimi (mirrors MainWindow.xaml.cs Render/SetCard/RenderExtra) ----

function setCard(prefix: string, seg: QuotaSegment | null | undefined): void {
  const pct = el(`${prefix}-pct`);
  const fill = el(`${prefix}-fill`);
  const reset = el(`${prefix}-reset`);
  if (!seg) {
    pct.textContent = '--';
    fill.style.width = '0%';
    reset.textContent = '';
    return;
  }
  // The unit lives in the markup next to the number (D layout), so only the
  // rounded value is written here.
  pct.textContent = String(Math.round(seg.percent));
  fill.style.width = `${clampPercent(seg.percent)}%`;
  reset.textContent = seg.resetAt ? formatReset(seg.resetAt) : '';
}

function renderExtra(extra: ExtraInfo | null | undefined): void {
  const balance = el('extra-balance');
  const monthlyPanel = el('extra-monthly');
  if (!extra) {
    balance.textContent = '--';
    monthlyPanel.hidden = true;
    return;
  }
  if (extra.state === 'Ready') {
    balance.textContent = extra.balanceCents != null ? fmtYuan(extra.balanceCents) : '--';
  } else if (extra.state === 'NoData') {
    balance.textContent = 'No data';
  } else {
    balance.textContent = 'Not activated';
  }
  if (extra.monthlyEnabled && extra.monthlyLimitCents != null && extra.monthlyLimitCents > 0 && extra.monthlyUsedCents != null) {
    const p = clampPercent((extra.monthlyUsedCents / extra.monthlyLimitCents) * 100);
    el('extra-fill').style.width = `${p}%`;
    el('extra-monthly-text').textContent =
      `Used ${fmtYuan(extra.monthlyUsedCents)} this month / ${fmtYuan(extra.monthlyLimitCents)} limit`;
    monthlyPanel.hidden = false;
  } else {
    monthlyPanel.hidden = true;
  }
}

function renderQuota(r: QuotaResult | null): void {
  quota = r;
  setCard('week', r?.week);
  setCard('five', r?.fiveHour);
  renderExtra(r?.extra);
  renderStamp();
  reportHeight();
}

// ---- Rendering: Kimi cache / decode figures (PLAN §5.1) ----
//
// These come from the CLI's own session logs, so the label says `local`: they
// cover this machine, not the account. Hidden entirely when no session has
// produced prompt tokens yet (a fresh install, or the first scan still running).

function renderCacheCell(
  prefix: 'week' | 'today',
  rate: number | null | undefined,
  window: KimiCacheWindow | undefined,
): void {
  const pct = el(`cache-${prefix}`);
  const fill = el(`cache-${prefix}-fill`);
  const note = el(`cache-${prefix}-note`);
  const num = el(`cache-${prefix}-num`);
  const requests = window?.requests ?? 0;
  note.textContent = requests > 0 ? `${fmtCount(requests)} requests` : 'No requests';
  if (rate == null) {
    pct.textContent = '--';
    fill.style.width = '0%';
    num.classList.remove('is-danger');
    return;
  }
  pct.textContent = fmtPct(rate) ?? '--';
  fill.style.width = `${clampPercent(rate)}%`;
  // Same threshold as the DeepSeek pane: a low hit rate is a failure, not a
  // gradient (PLAN §5.2).
  num.classList.toggle('is-danger', isLowCacheHit(rate));
}

function renderKimiUsage(u: KimiUsage | null): void {
  kimiUsage = u;
  const block = el('kimi-cache');
  const speed = el('kimi-speed');
  const week = u?.week?.hitRate ?? null;
  const today = u?.today?.hitRate ?? null;
  if (!u || (week == null && today == null)) {
    block.hidden = true;
    speed.hidden = true;
    reportHeight();
    return;
  }
  block.hidden = false;
  renderCacheCell('week', week, u.week);
  renderCacheCell('today', today, u.today);

  const tps = u.week?.decodeTps ?? null;
  if (tps == null) {
    speed.hidden = true;
  } else {
    speed.hidden = false;
    el('speed-value').textContent = `${tps.toFixed(1)} tok/s`;
    const first = u.week?.firstTokenMs;
    const calls = fmtCount(u.week?.calls ?? 0);
    el('speed-note').textContent =
      first != null
        ? `First token ${(first / 1000).toFixed(2)} s · ${calls} calls`
        : `${calls} calls`;
  }
  reportHeight();
}

function renderVersion(u: UpdateStatus): void {
  el('cli-version').textContent = u.localVersion ?? 'Not detected';
  el('badge').hidden = !u.updateAvailable;
  reportHeight();
}

// ---- Rendering: DeepSeek (PLAN §3.1 / §5.2) ----

/** Without an API key nothing can work, so that one reads as configuration
    rather than as a failed fetch; every other code is a failure. */
const NO_API_KEY = 'no-api-key';

function balanceStatusText(code: string | null | undefined): string {
  switch (code) {
    case null:
    case undefined:
      return '';
    case NO_API_KEY:
      return 'Not configured';
    case 'key-invalid':
      return 'Key invalid';
    case 'rate-limited':
      return 'Rate limited';
    default:
      return 'Fetch failed';
  }
}

function usageHintText(code: string): string {
  switch (code) {
    case 'no-user-token':
      // No "balance only" suffix: the balance block above already says whether
      // it worked, and with both credentials missing it contradicts itself.
      return 'Usage token not configured';
    case 'token-expired':
      return 'Usage token expired — sign in again to sync';
    case 'rate-limited':
      return 'Usage was rate limited — retrying in 30s';
    default:
      return 'Usage fetch failed — retrying in 30s';
  }
}

/** One model row: name + month cost, token-share bar, tokens/cache + today. */
function modelRow(m: DeepseekModelUsage, maxTokens: number): HTMLElement {
  const row = document.createElement('div');
  row.className = 'mrow';

  const main = document.createElement('div');
  main.className = 'mrow-main';
  const name = document.createElement('span');
  name.className = 'mname';
  // Model names are external data: textContent only, never innerHTML.
  const label = m.displayName || m.model;
  name.textContent = label;
  name.title = label; // the row ellipses a long official name
  const cost = document.createElement('span');
  cost.className = 'mcost';
  cost.textContent = fmtMoney(m.monthCost);
  main.append(name, cost);

  const bar = document.createElement('div');
  bar.className = 'bar share';
  const fill = document.createElement('i');
  // Share by tokens, not money: the bar compares models with each other
  // (mirrors the WPF parent's MaxTokens basis).
  fill.style.width = `${clampPercent((m.monthTokens / maxTokens) * 100)}%`;
  bar.append(fill);

  const meta = document.createElement('div');
  meta.className = 'meta';
  const used = document.createElement('span');
  used.textContent = `${fmtTokens(m.monthTokens)} tok · `;
  const hit = document.createElement('span');
  hit.className = isLowCacheHit(m.cacheHitRate) ? 'hit is-bad' : 'hit';
  hit.textContent = fmtHitRate(m.cacheHitRate);
  used.append(hit);
  const today = document.createElement('span');
  today.textContent = `today ${fmtMoney(m.todayCost)}`;
  meta.append(used, today);

  row.append(main, bar, meta);
  return row;
}

function renderModels(usage: DeepseekUsage): void {
  const models = el('ds-models');
  const summary = el('ds-summary');
  const rows = usage.byModel;
  if (rows.length === 0) {
    models.className = 'hint';
    models.textContent = 'No usage data this month';
    models.hidden = false;
    summary.hidden = true;
    return;
  }
  const maxTokens = Math.max(1, ...rows.map((m) => m.monthTokens));
  models.className = 'mrows';
  models.replaceChildren(...rows.map((m) => modelRow(m, maxTokens)));
  models.hidden = false;
  el('ds-sum').textContent =
    `This month ${fmtMoney(usage.monthCost)} · ${fmtTokens(usage.monthTokens)} tok`;
  el('ds-sum-today').textContent = `today ${fmtMoney(usage.todayCost)}`;
  summary.hidden = false;
}

function renderDeepseek(r: DeepseekResult | null): void {
  deepseek = r;

  const totalNum = el('ds-total-num');
  const total = el('ds-total');
  const status = el('ds-status');
  const detail = el('ds-detail');
  const models = el('ds-models');
  const summary = el('ds-summary');

  // Balance and usage fail independently (PLAN §5.2): the balance block keeps
  // its own status, the model list keeps its own hint.
  const b = r?.balance ?? null;
  if (b) {
    total.textContent = fmtMoney(b.total, b.currency);
    totalNum.classList.toggle('is-danger', isLowBalance(b.total, b.isAvailable));
    status.textContent = b.isAvailable ? 'Available' : 'Insufficient balance';
    status.classList.toggle('bad', !b.isAvailable);
    detail.textContent = `Granted ${fmtMoney(b.granted, b.currency)} · Topped up ${fmtMoney(b.toppedUp, b.currency)}`;
    detail.hidden = false;
  } else {
    const code = r?.balanceError ?? null;
    total.textContent = '--';
    totalNum.classList.remove('is-danger');
    status.textContent = balanceStatusText(code);
    status.classList.toggle('bad', code != null);
    detail.textContent = code === NO_API_KEY ? 'API key not set' : '';
    detail.hidden = detail.textContent === '';
  }

  if (r?.usage) {
    renderModels(r.usage);
  } else if (r?.usageError) {
    models.className = 'hint bad';
    models.textContent = usageHintText(r.usageError);
    models.hidden = false;
    summary.hidden = true;
  } else {
    // Nothing fetched yet: no rows, no hint, no hairline.
    models.replaceChildren();
    models.hidden = true;
    summary.hidden = true;
  }

  renderStamp();
  reportHeight();
}

// ---- Header stamp: it belongs to whichever provider tab is on screen ----

function renderStamp(): void {
  const stamp = el('last-updated');
  let text = '';
  let bad = false;
  if (tab === 'kimi') {
    if (quota) {
      if (quota.error) {
        // "Fetch failed", not "Update failed": this is the quota fetch. The CLI
        // version check is a separate path (PLAN §5.3).
        text = 'Fetch failed';
        bad = true;
      } else {
        text = fmtUpdated(quota.fetchedAt);
      }
    }
  } else if (deepseek) {
    if (deepseek.balanceError === NO_API_KEY) {
      text = 'Not configured';
      bad = true;
    } else if (deepseek.balanceError || deepseek.usageError) {
      text = 'Fetch failed';
      bad = true;
    } else {
      text = fmtUpdated(deepseek.fetchedAt);
    }
  }
  stamp.textContent = text;
  stamp.classList.toggle('bad', bad);
}

// ---- Provider tab (PLAN §3.4 / §3.6) ----

function syncRadios(): void {
  document.querySelectorAll<HTMLInputElement>('input[name="provider"]').forEach((input) => {
    input.checked = input.value === tab;
  });
}

function applyPanes(): void {
  el('pane-kimi').hidden = tab !== 'kimi';
  el('pane-deepseek').hidden = tab !== 'deepseek';
}

function applyTab(): void {
  syncRadios();
  applyPanes();
}

// Tab-switch motion (PLAN §3.6). The numbers are WinUI's page-refresh set: the
// cell grows on the entrance curve (fast out, slow in) and shrinks on the exit
// curve (slow out, fast in); both panes cross-fade over the same 167ms on those
// two curves. The fade deliberately runs for the whole move instead of WinUI's
// 83ms `ControlFasterAnimationDuration`: the outgoing pane paints above the
// incoming one, so fading it early would expose a bare `--paper` band while the
// cell is still collapsing (DS → Kimi leaves ~190px of it for ~40ms). Windows
// never animates a top-level window's size frame by frame, so the window jumps
// once to the taller tab (invisible, see the CSS) and everything the eye follows
// happens inside the WebView.
const MOTION = {
  grow: 200, // Fluent durationNormal: 167ms is too tight for ~280px of travel
  shrink: 167, // ControlFastAnimationDuration
  fadeIn: 167,
  fadeOut: 167, // matches the shrink: the old content must cover the closing cell
  enter: 'cubic-bezier(0, 0, 0, 1)', // WinUI: fast out, slow in
  exit: 'cubic-bezier(1, 0, 1, 1)', // WinUI: slow out, fast in
};

// A switch owns the panes for ~200ms. `tabRun` is the same generation guard
// `hideGen` uses: timers and promises left over from a superseded switch must
// never settle the one on screen.
let tabRun = 0;
let tabSwitching = false;
let tabStarted = false;
let tabAnims: Animation[] = [];

const reducedMotion = (): boolean =>
  window.matchMedia('(prefers-reduced-motion: reduce)').matches;

function nextFrame(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

/** Drop every trace of an in-flight switch without deciding what shows. */
function resetTabMotion(): void {
  tabSwitching = false;
  tabStarted = false;
  // Supersede a coroutine still parked on a frame: the fallback timer can settle
  // a switch while the panel is hidden (rAF pauses), and without this the
  // coroutine would wake up later and replay a stale pair of heights into the
  // freshly shown window.
  tabRun += 1;
  for (const anim of tabAnims) anim.cancel();
  tabAnims = [];
  const panes = document.querySelector<HTMLElement>('.panes');
  if (panes) panes.style.height = '';
  for (const id of ['pane-kimi', 'pane-deepseek']) {
    const pane = el(id);
    pane.style.opacity = '';
    pane.classList.remove('pane-leaving');
  }
}

/** Snap to the current `tab`. This is the end of a switch and the escape hatch
    for a second click that lands while the first one is still running. It fixes
    the panes only: the radio is owned by `switchTab`, which has already moved on
    to the new tab by then — re-syncing it here would snap the segmented control
    back to the tab the user just left. */
function settleTab(): void {
  if (!tabSwitching) return;
  resetTabMotion();
  applyPanes();
  reportHeight();
}

function switchTab(next: ProviderTab): void {
  if (next === tab) return;
  settleTab(); // a click inside the previous 200ms lands first, without motion
  const prev = tab;
  tab = next;
  // The segmented control follows the click immediately (that IS the feedback);
  // the header stamp belongs to the content, so it flips with the panes instead
  // (see playTabSwitch) — otherwise the stamp reads "DeepSeek" for the frames
  // the body is still Kimi.
  syncRadios();
  invoke('set_active_tab', { tab }).catch(() => undefined);

  const panes = document.querySelector<HTMLElement>('.panes');
  const root = document.getElementById('root');
  if (!panes || !root || reducedMotion()) {
    applyPanes();
    renderStamp();
    reportHeight();
    return;
  }
  // The generation is bumped here, synchronously, so the rejection handler below
  // can tell "my switch failed" from "a newer switch is running and I must not
  // touch it" — the one recovery path a bare `catch` would get wrong.
  const run = ++tabRun;
  playTabSwitch(run, prev, next, panes, root).catch(() => {
    if (tabSwitching && run === tabRun) settleTab();
  });
}

async function playTabSwitch(
  run: number,
  prev: ProviderTab,
  next: ProviderTab,
  panes: HTMLElement,
  root: HTMLElement,
): Promise<void> {
  const outgoing = el(prev === 'kimi' ? 'pane-kimi' : 'pane-deepseek');
  const incoming = el(next === 'kimi' ? 'pane-kimi' : 'pane-deepseek');

  // Both layouts are measured inside one task, so nothing paints in between:
  // the outgoing pane leaves the flow first, which lets the incoming one alone
  // decide the height (PLAN §3.6 — an in-flow pair measures the taller pane and
  // would size the window for the tab that is not on screen).
  const h0 = root.offsetHeight;
  const p0 = panes.offsetHeight;
  // Header + footer + the card's 2px border, measured off the CONTENT height:
  // `h0` is the capped box once `.clamped` is on, and `h0 - p0` would then
  // under-report the chrome and make every later height too small.
  const chrome = root.scrollHeight + 2 - p0;
  outgoing.classList.add('pane-leaving');
  incoming.hidden = false;
  incoming.style.opacity = '0';
  panes.style.height = `${p0}px`; // pin at the old height while the card animates
  tabSwitching = true;
  // With `.panes` pinned, the live height of the incoming tab has to be asked of
  // the pane itself. Read at use time, never once: a poll tick can add rows while
  // the window resize is in flight, and a stale end keyframe would clip the new
  // rows until the settle snaps them in.
  const cardHeight = (): number => chrome + incoming.offsetHeight;

  // Window first, content second: it has to be tall enough for the card to grow
  // into before the first animated frame, or the card's top edge — and the tab
  // the user just clicked — gets cut by the window edge. The extra room appears
  // above a card whose bottom edge is anchored, so this jump cannot be seen.
  // A switch cut short by a hide still has to settle: rAF pauses for a hidden
  // page, so the frame awaited below may not arrive until the panel is visible
  // again. Only a coroutine that never reached the animation is rescued here —
  // a slow IPC that eventually starts one must not be cut off mid-flight (the
  // animation has its own end timer). `settleTab()` is idempotent, and a newer
  // switch bumps `run` first.
  window.setTimeout(() => {
    if (run === tabRun && !tabStarted) settleTab();
  }, MOTION.grow + 260);
  let target = Math.max(h0, cardHeight()) + SHADOW_ROOM;
  await invoke('panel_height', { height: target }).catch(() => undefined);
  if (run !== tabRun) return;
  // The IPC reply means the size was applied, not that the WebView has resized
  // its viewport yet; a card that grows first gets clipped at the top. Wait for
  // the real viewport, capped so a stuck resize cannot stall the switch forever.
  for (let i = 0; i < 3 && window.innerHeight + 1 < target; i += 1) await nextFrame();
  if (run !== tabRun) return;
  // Content that arrived during the resize belongs to this animation, not to a
  // snap at the end: grow the window once more if the tab got taller meanwhile.
  const retarget = Math.max(h0, cardHeight()) + SHADOW_ROOM;
  if (retarget > target + 0.5) {
    target = retarget;
    await invoke('panel_height', { height: target }).catch(() => undefined);
    for (let i = 0; i < 3 && window.innerHeight + 1 < target; i += 1) await nextFrame();
    if (run !== tabRun) return;
  }
  // If the backend had to clamp the window (small work area), the card has to
  // cap itself BEFORE it grows past the window's top edge — the steady-state
  // toggle only runs at the settle, which would leave the header and the
  // provider tab clipped for the whole animation. The max matters: shrinking
  // FROM an overflowing tab stays capped until the animation has come down.
  root.classList.toggle(
    'clamped',
    Math.max(chrome + p0, cardHeight()) + SHADOW_ROOM > window.innerHeight + 1,
  );

  const end = incoming.offsetHeight;
  const grow = end > p0;
  // The header stamp belongs to the pane on screen, so it flips with it — not
  // with the radio, which moved the moment the user clicked.
  renderStamp();
  tabStarted = true;
  const card = panes.animate([{ height: `${p0}px` }, { height: `${end}px` }], {
    duration: grow ? MOTION.grow : MOTION.shrink,
    easing: grow ? MOTION.enter : MOTION.exit,
    fill: 'forwards',
  });
  const out = outgoing.animate([{ opacity: 1 }, { opacity: 0 }], {
    // Never longer than the cell's own move: `settleTab()` hides the pane when
    // the cell lands, so a fade still running then would pop. The invariant is
    // shrink <= fadeOut <= grow; the shrink matches, the grow has headroom.
    duration: Math.min(MOTION.fadeOut, grow ? MOTION.grow : MOTION.shrink),
    easing: MOTION.exit,
    fill: 'forwards',
  });
  const into = incoming.animate([{ opacity: 0 }, { opacity: 1 }], {
    duration: MOTION.fadeIn,
    easing: MOTION.enter,
    fill: 'forwards',
  });
  tabAnims = [card, out, into];

  const done = (): void => {
    if (run !== tabRun) return;
    settleTab();
  };
  card.finished.then(done).catch(() => undefined);
  // Timers keep firing when a hidden window throttles rAF, so hiding the panel
  // mid-switch can never leave the panes pinned at a stale height.
  window.setTimeout(done, (grow ? MOTION.grow : MOTION.shrink) + 60);
}

// ---- Content height reporting (PLAN §4) ----

// Measured synchronously, never through requestAnimationFrame: the panel
// renders while its window is still hidden (startup, and every quota-updated
// tick while hidden), and Chromium pauses rAF for hidden pages — an rAF-based
// report would only land after the window became visible, so the first show
// would use a stale height. Reading scrollHeight forces the layout we need.
// Only the active pane is laid out, so this is always the height of the tab on
// screen — except during a tab switch, where the window is deliberately held at
// the taller of the two tabs and `settleTab()` reports the final height itself.
function reportHeight(): void {
  if (tabSwitching) return;
  const root = document.getElementById('root');
  if (!root) return;
  // Report the CONTENT height, not the box: once `.clamped` is on, offsetHeight
  // is the cap and the backend would follow it down to nothing.
  const natural = root.scrollHeight + 2 + SHADOW_ROOM;
  invoke('panel_height', { height: natural })
    .catch(() => undefined)
    .then(() => {
      // Compare against the viewport AFTER the resize has landed: comparing
      // before it makes every growing tick flash a scrollbar for a frame (and
      // reflow the two columns by its width). The backend clamps to the work
      // area, so a genuinely too-short window still shows up here.
      requestAnimationFrame(() => {
        // Re-measure at frame time: several reports can land in one tick
        // (quota + cache), and a `natural` captured before the IPC round trip
        // would flip the flag onto content that has already changed again.
        const live = root.scrollHeight + 2 + SHADOW_ROOM;
        root.classList.toggle('clamped', live > window.innerHeight + 1);
      });
    });
}

// ---- Show/hide animation (SPEC 15.1 / 15.2) ----

// Generation counter: a pending hide timer must not fire after a newer
// show/hide, or a rapid hide→show→hide sequence would cut the last hide
// animation short.
let hideGen = 0;

function playShow(): void {
  hideGen++; // supersede any pending hide timer
  document.body.classList.remove('leave');
  // Force reflow so the enter transition always restarts from the initial state
  void document.body.offsetWidth;
  document.body.classList.add('enter');
  reportHeight(); // keep the bottom-anchored size in step with content
}

function playHide(): void {
  const gen = ++hideGen;
  document.body.classList.remove('enter');
  document.body.classList.add('leave');
  // Fade-out takes 130ms; tell the backend to actually hide the window afterwards
  window.setTimeout(() => {
    if (gen !== hideGen) return; // superseded by a newer show/hide
    invoke('finish_hide_panel').catch(() => undefined);
    document.body.classList.remove('leave');
  }, 170);
}

// ---- Bootstrap ----

window.addEventListener('DOMContentLoaded', async () => {
  // Register listeners BEFORE any other await (including initTheme's) so an
  // early backend event — panel-show, quota-updated, deepseek-updated — can
  // never be missed.
  await listen<QuotaResult>('quota-updated', (e) => renderQuota(e.payload));
  await listen<DeepseekResult>('deepseek-updated', (e) => renderDeepseek(e.payload));
  await listen<KimiUsage>('kimi-usage-updated', (e) => renderKimiUsage(e.payload));
  await listen<UpdateStatus>('update-status', (e) => renderVersion(e.payload));
  await listen('panel-show', () => playShow());
  await listen('panel-hide', () => playHide());

  let state: AppStateDto;
  try {
    state = await initTheme();
  } catch {
    return;
  }
  // The persisted tab decides which pane is on screen before the first
  // measurement, so the window is never sized for the other provider (PLAN §4).
  tab = state.settings.activeTab === 'deepseek' ? 'deepseek' : 'kimi';
  applyTab();
  renderQuota(state.quota);
  renderVersion(state.update);
  // Re-report once fonts have settled: the first measurement can precede the
  // final line heights and leave the window a few pixels off.
  window.setTimeout(reportHeight, 120);

  // The DeepSeek half of the initial state; `get_state` stays Kimi-sized
  // because the settings and skills windows do not need the usage list.
  invoke<DeepseekResult | null>('get_deepseek')
    .then((r) => renderDeepseek(r))
    .catch(() => undefined);
  // Same for the local cache figures: the first scan may still be running, in
  // which case the block simply stays hidden until `kimi-usage-updated`.
  invoke<KimiUsage | null>('get_kimi_usage')
    .then((u) => renderKimiUsage(u))
    .catch(() => undefined);

  document.querySelectorAll<HTMLInputElement>('input[name="provider"]').forEach((input) => {
    input.addEventListener('change', () => {
      if (input.checked) switchTab(input.value === 'deepseek' ? 'deepseek' : 'kimi');
    });
  });

  el('btn-console').addEventListener('click', () => {
    invoke('open_console').catch(() => undefined);
  });
  el('btn-refresh').addEventListener('click', () => {
    invoke('refresh_now').catch(() => undefined);
  });
  el('btn-settings').addEventListener('click', () => {
    invoke('open_settings').catch(() => undefined);
  });
  el('btn-exit').addEventListener('click', () => {
    invoke('quit_app').catch(() => undefined);
  });
  el('version-row').addEventListener('click', () => {
    invoke('open_releases').catch(() => undefined);
  });

  // A recovery reload rebuilds this page while its window stays visible, and no
  // panel-show follows a reload: #root would sit at opacity 0 inside a window
  // the user can still see and click. Paint the first frame only in that case —
  // at startup the window is hidden, and the fade must still play on the first
  // real show (PLAN §3.6).
  if (await getCurrentWindow().isVisible().catch(() => false)) {
    playShow();
  }
});
