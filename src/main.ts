import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
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

function applyTab(animate: boolean): void {
  document.querySelectorAll<HTMLInputElement>('input[name="provider"]').forEach((input) => {
    input.checked = input.value === tab;
  });
  const kimi = el('pane-kimi');
  const ds = el('pane-deepseek');
  kimi.hidden = tab !== 'kimi';
  ds.hidden = tab !== 'deepseek';
  const shown = tab === 'kimi' ? kimi : ds;
  shown.classList.remove('tab-in');
  if (animate) {
    void shown.offsetWidth; // restart the keyframes on a repeated switch
    shown.classList.add('tab-in');
  }
}

function switchTab(next: ProviderTab): void {
  if (next === tab) return;
  tab = next;
  applyTab(true);
  renderStamp();
  // Height changes in the same frame as the content, and the backend re-pins
  // the card's bottom-right corner while resizing (PLAN §4).
  reportHeight();
  invoke('set_active_tab', { tab }).catch(() => undefined);
}

// ---- Content height reporting (PLAN §4) ----

// Measured synchronously, never through requestAnimationFrame: the panel
// renders while its window is still hidden (startup, and every quota-updated
// tick while hidden), and Chromium pauses rAF for hidden pages — an rAF-based
// report would only land after the window became visible, so the first show
// would use a stale height. Reading offsetHeight forces the layout we need.
// Only the active pane is laid out, so this is always the height of the tab on
// screen.
function reportHeight(): void {
  const root = document.getElementById('root');
  if (!root) return;
  invoke('panel_height', { height: root.offsetHeight + SHADOW_ROOM }).catch(() => undefined);
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
  applyTab(false);
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
});
