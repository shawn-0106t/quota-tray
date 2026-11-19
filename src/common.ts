import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// ---- Shared IPC types (mirrors src-tauri structs, camelCase) ----

export interface QuotaSegment {
  percent: number;
  resetAt?: string | null;
}

export type ExtraState = 'NotActivated' | 'NoData' | 'Ready';

export interface ExtraInfo {
  state: ExtraState;
  balanceCents?: number | null;
  monthlyEnabled: boolean;
  monthlyUsedCents?: number | null;
  monthlyLimitCents?: number | null;
}

export interface QuotaResult {
  fiveHour?: QuotaSegment | null;
  week?: QuotaSegment | null;
  extra?: ExtraInfo | null;
  fetchedAt: string;
  error?: string | null;
}

export interface UpdateStatus {
  localVersion?: string | null;
  latestVersion?: string | null;
  updateAvailable: boolean;
  checkFailed: boolean;
}

/** DeepSeek payload (PLAN §5.2). Mirrors deepseek/mod.rs, camelCase. */
export interface DeepseekBalance {
  isAvailable: boolean;
  currency: string;
  total: number;
  granted: number;
  toppedUp: number;
}

export interface DeepseekModelUsage {
  model: string;
  /** Official vendor name when known; already resolved by the backend. */
  displayName: string;
  monthTokens: number;
  monthRequests: number;
  monthCost: number;
  /** 0-100; null when the model has no prompt tokens at all. */
  cacheHitRate?: number | null;
  todayTokens: number;
  todayCost: number;
}

export interface DeepseekUsage {
  monthTokens: number;
  monthRequests: number;
  monthCost: number;
  cacheHitRate?: number | null;
  todayTokens: number;
  todayCost: number;
  /** Already filtered and capped at four by the backend: draw as-is. */
  byModel: DeepseekModelUsage[];
}

export interface DeepseekResult {
  balance?: DeepseekBalance | null;
  usage?: DeepseekUsage | null;
  /** Balance and usage fail independently (PLAN §5.2): one code each. */
  balanceError?: string | null;
  usageError?: string | null;
  fetchedAt: string;
}

// ---- Kimi local statistics (PLAN §5.1) ----
//
// Read from the CLI's own session logs, not from the quota API: `local` is part
// of the label in the pane, because these numbers cover this machine only.

export interface KimiCacheWindow {
  requests: number;
  /** Percent, 0-100; null when no prompt tokens were seen in the window */
  hitRate?: number | null;
  /** Output tokens per second of server decode time */
  decodeTps?: number | null;
  /** Mean time to first token, milliseconds */
  firstTokenMs?: number | null;
  /** Calls behind the two speed figures */
  calls: number;
}

export interface KimiUsage {
  week: KimiCacheWindow;
  today: KimiCacheWindow;
  scannedAt: string;
  files: number;
  scanMs: number;
}

export type ProviderTab = 'kimi' | 'deepseek';

export interface SettingsDto {
  theme: string; // system | light | dark
  refreshMinutes: number;
  autoStart: boolean;
  activeTab: string; // kimi | deepseek
}

export interface AppStateDto {
  quota: QuotaResult | null;
  update: UpdateStatus;
  settings: SettingsDto;
  theme: string; // effective theme: light | dark
}

// ---- Settings window: read-only credential status (PLAN §4) ----
// Statuses only — a token or an API key never crosses this boundary.

export interface KimiCredentialStatus {
  /** oauth | api-key | none */
  source: string;
  /** Unix seconds, OAuth only */
  expiresAt?: number | null;
  /** An OAuth entry exists but is unusable and nothing took over */
  expired: boolean;
}

export interface DeepseekCredentialStatus {
  apiKey: boolean;
  userToken: boolean;
  /** env | file | imported | undecryptable | none */
  source: string;
}

export interface CredentialStatus {
  kimi: KimiCredentialStatus;
  deepseek: DeepseekCredentialStatus;
  cliVersion?: string | null;
  latestVersion?: string | null;
  updateAvailable: boolean;
}

/** Transparent shadow-fade room around a window card: 28px on each side (a
    margin in the settings/skills/menu windows, the `inset` of the panel's
    bottom-anchored card). Must match PANEL_MARGIN in src-tauri/src/panel.rs. */
export const SHADOW_ROOM = 56;

// ---- Theme ----

export function applyTheme(theme: string): void {
  document.documentElement.dataset.theme = theme === 'dark' ? 'dark' : 'light';
}

/** Apply current effective theme and subscribe to live changes. */
export async function initTheme(): Promise<AppStateDto> {
  // Subscription first: `get_state` is an IPC round trip and a theme change
  // landing during it would be lost.
  const subscription = listen<string>('theme-changed', (e) => applyTheme(e.payload));
  const state = await invoke<AppStateDto>('get_state');
  applyTheme(state.theme);
  await subscription;
  return state;
}

// ---- Formatting (PLAN §5.2: money is always two decimals) ----
//
// The Slint shell has no TypeScript and uses the Rust port in
// `quota-core/src/format.rs` instead. The two are pinned together by
// `spike/p1/fmt_parity.py` (45 cases, string for string): change one and run it.

/** FormatReset: span = at - now, English countdown text. */
export function formatReset(atIso: string): string {
  const spanMs = new Date(atIso).getTime() - Date.now();
  if (Number.isNaN(spanMs)) return '';
  if (spanMs < 0) return 'Resets soon';
  const totalSec = Math.floor(spanMs / 1000);
  const days = Math.floor(totalSec / 86400);
  if (days >= 1) {
    const hours = Math.floor(totalSec / 3600) % 24;
    return `Resets in ${days}d ${hours}h`;
  }
  const totalHours = Math.floor(totalSec / 3600);
  if (totalHours >= 1) {
    const minutes = Math.floor(totalSec / 60) % 60;
    return `Resets in ${totalHours}h ${minutes}m`;
  }
  const minutes = Math.floor(totalSec / 60);
  return `Resets in ${Math.max(1, minutes)}m`;
}

/** FmtYuan: cents -> ¥, always two decimals — the panel never shows `¥10` or
    `¥8.2` next to `¥49.52` (user decision 2026-11-14). */
export function fmtYuan(cents: number): string {
  if (cents < 0) return '-' + fmtYuan(-cents);
  const yuan = Math.floor(cents / 100);
  const frac = cents % 100;
  return `¥${yuan}.${String(frac).padStart(2, '0')}`;
}

export function clampPercent(percent: number): number {
  return Math.min(100, Math.max(0, percent));
}

// ---- DeepSeek formatting and thresholds (PLAN §3.1 / §5.2) ----
//
// The DeepSeek payload carries money in YUAN (snapped to a 1e-8 yuan quantum),
// unlike Kimi's cents -> use fmtMoney, not fmtYuan.

/** Balance under this many yuan is a failure, not a number (PLAN §5.2). */
export const LOW_BALANCE_YUAN = 10;
/** Cache hit rate under this percentage is a failure (PLAN §5.2). */
export const LOW_CACHE_HIT = 60;

/** FmtMoney: DeepSeek money is yuan (not cents) — always two decimals, same
    rule as fmtYuan (user decision 2026-11-14). */
export function fmtMoney(yuan: number, currency = 'CNY'): string {
  const symbol = currency === 'USD' ? '$' : '¥';
  return symbol + yuan.toFixed(2);
}

/** FmtTokens: 1.33M / 284K / 96, two decimals at most and no trailing zeros. */
export function fmtTokens(tokens: number): string {
  const trim = (s: string): string => s.replace(/\.?0+$/, '');
  if (tokens >= 1_000_000) return `${trim((tokens / 1_000_000).toFixed(2))}M`;
  if (tokens >= 1_000) return `${trim((tokens / 1_000).toFixed(1))}K`;
  return String(tokens);
}

/** `cache 80%` (one decimal, no trailing zero); `cache --` when unknown. */
export function fmtHitRate(rate: number | null | undefined): string {
  if (rate == null) return 'cache --';
  return `cache ${rate.toFixed(1).replace(/\.0$/, '')}%`;
}

/** PLAN §5.2: an unavailable or nearly empty balance is red. */
export function isLowBalance(total: number, isAvailable: boolean): boolean {
  return !isAvailable || total < LOW_BALANCE_YUAN;
}

/** PLAN §5.2: a cache hit rate under 60% is red — low means wasted cache. */
export function isLowCacheHit(rate: number | null | undefined): boolean {
  return rate != null && rate < LOW_CACHE_HIT;
}

/** One decimal, trailing `.0` dropped: `99.4`, `100`. */
export function fmtPct(value: number | null | undefined): string | null {
  if (value == null) return null;
  return value.toFixed(1).replace(/\.0$/, '');
}

/** Thousands separator for request / call counts: `3,097`. */
export function fmtCount(value: number): string {
  return value.toLocaleString('en-US');
}

/** Panel stamp text for a fetch time: `Updated HH:MM` in local time, prefixed
 *  `M/D ` when the fetch happened on an earlier local day — a cache reused
 *  across a restart must not read as if it were fetched today (PLAN §5.2). */
export function fmtUpdated(atIso: string): string {
  const d = new Date(atIso);
  if (Number.isNaN(d.getTime())) return '';
  const hh = String(d.getHours()).padStart(2, '0');
  const mm = String(d.getMinutes()).padStart(2, '0');
  const now = new Date();
  const sameDay =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();
  const day = sameDay ? '' : `${d.getMonth() + 1}/${d.getDate()} `;
  return `Updated ${day}${hh}:${mm}`;
}
