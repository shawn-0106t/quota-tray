import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './theme.css';
import './main.css';
import {
  AppStateDto,
  ExtraInfo,
  QuotaResult,
  QuotaSegment,
  SHADOW_ROOM,
  UpdateStatus,
  clampPercent,
  fmtYuan,
  formatReset,
  initTheme,
} from './common';

function el<T extends HTMLElement>(id: string): T {
  return document.getElementById(id) as T;
}

// ---- Rendering (mirrors MainWindow.xaml.cs Render/SetCard/RenderExtra) ----

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
  setCard('week', r?.week);
  setCard('five', r?.fiveHour);
  renderExtra(r?.extra);
  const lastUpdated = el('last-updated');
  if (!r) {
    lastUpdated.textContent = '';
    lastUpdated.classList.remove('bad');
  } else if (r.error) {
    lastUpdated.textContent = 'Update failed';
    lastUpdated.classList.add('bad');
  } else {
    const d = new Date(r.fetchedAt);
    const hh = String(d.getHours()).padStart(2, '0');
    const mm = String(d.getMinutes()).padStart(2, '0');
    lastUpdated.textContent = `Updated ${hh}:${mm}`;
    lastUpdated.classList.remove('bad');
  }
  reportHeight();
}

function renderVersion(u: UpdateStatus): void {
  el('cli-version').textContent = u.localVersion ?? 'Not detected';
  el('badge').hidden = !u.updateAvailable;
  reportHeight();
}

// ---- Content height reporting (PLAN §4) ----

// Measured synchronously, never through requestAnimationFrame: the panel
// renders while its window is still hidden (startup, and every quota-updated
// tick while hidden), and Chromium pauses rAF for hidden pages — an rAF-based
// report would only land after the window became visible, so the first show
// would use a stale height. Reading offsetHeight forces the layout we need.
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
  // early backend event — panel-show, quota-updated — can never be missed.
  await listen<QuotaResult>('quota-updated', (e) => renderQuota(e.payload));
  await listen<UpdateStatus>('update-status', (e) => renderVersion(e.payload));
  await listen('panel-show', () => playShow());
  await listen('panel-hide', () => playHide());

  let state: AppStateDto;
  try {
    state = await initTheme();
  } catch {
    return;
  }
  renderQuota(state.quota);
  renderVersion(state.update);
  // Re-report once fonts have settled: the first measurement can precede the
  // final line heights and leave the window a few pixels off.
  window.setTimeout(reportHeight, 120);

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
