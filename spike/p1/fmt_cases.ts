// Cross-shell formatting probe (P1 evidence, not part of the product).
// Emits the same cases the Rust probe computes, plus the inputs it needs.
// Driven by spike/p1/fmt_parity.py; writes cases.json + js.tsv into the CWD.
import {
  fmtYuan,
  fmtMoney,
  fmtTokens,
  fmtHitRate,
  fmtPct,
  fmtCount,
  fmtUpdated,
  formatReset,
  clampPercent,
  isLowBalance,
  isLowCacheHit,
} from './common_probe.ts';

import { writeFileSync } from 'node:fs';

type Case = { label: string; fn: string; arg?: unknown; result?: string };

const cases: Case[] = [];
const push = (label: string, fn: string, arg: unknown, result: string): void => {
  cases.push({ label, fn, arg, result });
};

for (const cents of [0, 5, 100, 1050, 4952, -150]) push(`fmtYuan(${cents})`, 'fmtYuan', cents, fmtYuan(cents));
for (const [yuan, cur] of [[45.44, 'CNY'], [0, 'CNY'], [3, 'CNY'], [1.5, 'USD'], [0.125, 'CNY'], [0.375, 'USD']] as const) {
  push(`fmtMoney(${yuan},${cur})`, 'fmtMoney', [yuan, cur], fmtMoney(yuan, cur));
}
for (const n of [0, 96, 999, 1000, 1250, 284000, 1125000, 1330000, 1000000]) push(`fmtTokens(${n})`, 'fmtTokens', n, fmtTokens(n));
for (const r of [null, 80, 59.25, 99.5, 100]) push(`fmtHitRate(${r})`, 'fmtHitRate', r, fmtHitRate(r));
for (const v of [null, 100, 42.25, 99.44]) push(`fmtPct(${v})`, 'fmtPct', v, fmtPct(v) ?? '<null>');
for (const n of [0, 96, 3097, 1234567, -3097]) push(`fmtCount(${n})`, 'fmtCount', n, fmtCount(n));
for (const p of [-5, 120, 42.5]) push(`clampPercent(${p})`, 'clampPercent', p, String(clampPercent(p)));
for (const [total, avail] of [[9.99, true], [10, true], [500, false]] as const) {
  push(`isLowBalance(${total},${avail})`, 'isLowBalance', [total, avail], String(isLowBalance(total, avail)));
}
for (const r of [null, 59.9, 60]) push(`isLowCacheHit(${r})`, 'isLowCacheHit', r, String(isLowCacheHit(r)));

// Date cases: hand the Rust side the exact instants, so both sides agree on
// "now" to the second.
const now = new Date();
const nowIso = now.toISOString();
const offsets: [string, number][] = [
  ['past', -60_000],
  ['30s', 30_000],
  ['42m', 42 * 60_000 + 10_000],
  ['2h5m', (2 * 60 + 5) * 60_000 + 10_000],
  ['2d3h', ((2 * 24 + 3) * 60 + 10) * 60_000],
];
for (const [label, ms] of offsets) {
  const atIso = new Date(now.getTime() + ms).toISOString();
  push(`formatReset(${label})`, 'formatReset', [atIso, nowIso], formatReset(atIso));
}
const today = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 9, 5, 0);
const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1, 23, 59, 0);
for (const [label, d] of [['today', today], ['yesterday', yesterday]] as const) {
  push(`fmtUpdated(${label})`, 'fmtUpdated', [d.toISOString(), nowIso], fmtUpdated(d.toISOString()));
}

writeFileSync('cases.json', JSON.stringify(cases, null, 1), 'utf8');
const tsv = cases.map((c) => `${c.label}\t${c.result}`).join('\n');
writeFileSync('js.tsv', tsv + '\n', 'utf8');
console.log(tsv);
