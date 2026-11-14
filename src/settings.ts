import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './theme.css';
import './settings.css';
import { AppStateDto, CredentialStatus, SHADOW_ROOM, initTheme } from './common';

function el<T extends HTMLElement>(id: string): T {
  return document.getElementById(id) as T;
}

/** Report the content height so the backend can size and re-centre the
    window (PLAN §4). The card is content-sized, so this needs no constant. */
function reportHeight(): void {
  const root = document.getElementById('root');
  if (!root) return;
  invoke('settings_height', { height: root.offsetHeight + SHADOW_ROOM }).catch(
    () => undefined,
  );
}

/** Backfill the current settings into the controls (SPEC 13.2: every open). */
async function backfill(): Promise<void> {
  try {
    const state = await invoke<AppStateDto>('get_state');
    const s = state.settings;
    // Iterate instead of interpolating into a CSS selector: a malformed
    // persisted theme value must not throw and leave every control blank.
    let themeMatched = false;
    document.querySelectorAll<HTMLInputElement>('input[name="theme"]').forEach((i) => {
      i.checked = i.value === s.theme;
      themeMatched ||= i.checked;
    });
    if (!themeMatched) {
      const fallback = document.querySelector<HTMLInputElement>(
        'input[name="theme"][value="system"]',
      );
      if (fallback) fallback.checked = true;
    }
    const wanted = String(s.refreshMinutes);
    document.querySelectorAll<HTMLInputElement>('input[name="interval"]').forEach((i) => {
      i.checked = i.value === wanted;
    });
    el<HTMLInputElement>('autostart').checked = s.autoStart;
  } catch {
    /* backend unavailable: keep current control state */
  }
}

// ---- Read-only credential sections (PLAN §4) ----
//
// Statuses only: the backend never sends a token or an API key here, so there
// is nothing to mask. Red marks something broken (expired, unreadable), not
// something merely missing — the panel header already reports a failing fetch.

/** HH:MM for the OAuth expiry; these tokens live for hours, so the date never
    matters and a time-only stamp reads better. */
function fmtClock(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  if (Number.isNaN(d.getTime())) return '';
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

function setValue(node: HTMLElement, text: string, bad = false): void {
  node.textContent = text;
  node.classList.toggle('bad', bad);
}

function renderKimi(
  kimi: CredentialStatus['kimi'],
  cliVersion: string | null | undefined,
  latestVersion: string | null | undefined,
  updateAvailable: boolean,
): void {
  const local = cliVersion ?? 'Not found';
  setValue(
    el('kimi-version'),
    updateAvailable && latestVersion ? `${local} → ${latestVersion}` : local,
  );
  if (kimi.source === 'oauth') {
    const expires = kimi.expiresAt ? ` · expires ${fmtClock(kimi.expiresAt)}` : '';
    setValue(el('kimi-credential'), `OAuth token${expires}`);
  } else if (kimi.source === 'api-key') {
    setValue(el('kimi-credential'), 'API key (config.toml)');
  } else if (kimi.expired) {
    setValue(el('kimi-credential'), 'Expired — sign in with the Kimi CLI', true);
  } else {
    setValue(el('kimi-credential'), 'Not found — sign in with the Kimi CLI', true);
  }
}

function renderDeepseek(ds: CredentialStatus['deepseek']): void {
  const describe = (configured: boolean): [string, boolean] => {
    if (configured) return ['Configured', false];
    // A blob that will not decrypt is a broken state (copied portable profile,
    // another Windows account): re-entry is the fix, so say so.
    if (ds.source === 'undecryptable') return ['Unreadable — re-enter', true];
    return ['Not configured', false];
  };
  const [keyText, keyBad] = describe(ds.apiKey);
  setValue(el('ds-api-key'), keyText, keyBad);
  const [tokenText, tokenBad] = describe(ds.userToken);
  setValue(el('ds-user-token'), tokenText, tokenBad);

  const note = el('ds-note');
  if (ds.source === 'env') {
    note.textContent =
      'Values come from DEEPSEEK_API_KEY / DEEPSEEK_USER_TOKEN — Clear does not remove them.';
    note.hidden = false;
  } else {
    note.textContent = '';
    note.hidden = true;
  }
}

async function refreshStatus(): Promise<void> {
  try {
    const status = await invoke<CredentialStatus>('credential_status');
    renderKimi(status.kimi, status.cliVersion, status.latestVersion, status.updateAvailable);
    renderDeepseek(status.deepseek);
  } catch {
    /* backend unavailable: keep the placeholders */
  }
  // Rows and the env note change the content height.
  reportHeight();
}

/** PLAN §5.2 error codes -> text the manual fallback can show. */
function credError(code: string): string {
  switch (code) {
    case 'key-invalid':
      return 'invalid key — not saved';
    case 'token-expired':
      return 'invalid or expired — not saved';
    case 'rate-limited':
      return 'rate limited — try again later';
    case 'network-error':
      return 'network error — not saved';
    case 'server-error':
      return 'DeepSeek server error — not saved';
    case 'empty':
      return 'empty value';
    default:
      return `verification failed (${code}) — not saved`;
  }
}

function showCredNote(text: string, bad: boolean): void {
  const note = el('cred-note');
  note.textContent = text;
  note.classList.toggle('bad', bad);
  note.hidden = false;
  reportHeight();
}

/** Manual fallback: verify each non-empty field, save only what passes. */
async function saveCredentials(): Promise<void> {
  const fields = [
    { field: 'apiKey', input: 'in-api-key', label: 'API key' },
    { field: 'userToken', input: 'in-user-token', label: 'Usage token' },
  ] as const;
  const values = fields
    .map((f) => ({ ...f, value: el<HTMLInputElement>(f.input).value.trim() }))
    .filter((f) => f.value.length > 0);
  if (values.length === 0) {
    showCredNote('Paste an API key or a usage token first.', true);
    return;
  }
  const button = el<HTMLButtonElement>('btn-save-cred');
  button.disabled = true;
  button.textContent = 'Verifying…';
  const problems: string[] = [];
  try {
    for (const item of values) {
      try {
        await invoke('save_credential', { field: item.field, value: item.value });
        el<HTMLInputElement>(item.input).value = '';
      } catch (error) {
        problems.push(`${item.label}: ${credError(String(error))}`);
      }
    }
  } finally {
    button.disabled = false;
    button.textContent = 'Save credentials';
  }
  if (problems.length > 0) {
    showCredNote(problems.join(' · '), true);
  } else {
    showCredNote('Verified and saved.', false);
  }
  await refreshStatus();
}

window.addEventListener('DOMContentLoaded', async () => {
  // Register before any await: the window is reused (hidden, not destroyed),
  // and a settings-show emitted during initTheme must not be missed (same
  // race as panel-show in main.ts).
  await listen('settings-show', () => {
    backfill().catch(() => undefined);
    refreshStatus().catch(() => undefined);
  });
  // Signing in or clearing changes the statuses while this window is open.
  await listen('deepseek-credentials-changed', () => {
    refreshStatus().catch(() => undefined);
  });

  try {
    await initTheme();
  } catch {
    /* backend unavailable: keep default theme */
  }
  await Promise.all([backfill(), refreshStatus()]);
  reportHeight();
  // Re-report once fonts have settled (the first measurement can precede the
  // final line heights).
  window.setTimeout(reportHeight, 120);

  el('btn-save').addEventListener('click', () => {
    const theme =
      document.querySelector<HTMLInputElement>('input[name="theme"]:checked')?.value ?? 'system';
    const refreshMinutes = Number(
      document.querySelector<HTMLInputElement>('input[name="interval"]:checked')?.value ?? '5',
    );
    const autoStart = el<HTMLInputElement>('autostart').checked;
    invoke('save_settings', { settings: { theme, refreshMinutes, autoStart } }).catch(
      () => undefined,
    );
  });

  el('btn-sign-in').addEventListener('click', () => {
    invoke('open_login').catch(() => undefined);
  });

  el('btn-clear').addEventListener('click', () => {
    invoke('clear_credentials').catch(() => undefined);
  });

  el('btn-save-cred').addEventListener('click', () => {
    saveCredentials().catch(() => undefined);
  });

  // The manual block is a disclosure: opening it grows the window.
  el('manual').addEventListener('toggle', () => {
    reportHeight();
  });

  el('btn-close').addEventListener('click', () => {
    invoke('close_settings').catch(() => undefined);
  });

  // Custom title bar drag (SPEC 13.1: DragMove on left button down)
  el('titlebar').addEventListener('mousedown', (e) => {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest('.chrome-close')) return;
    invoke('start_drag').catch(() => undefined);
  });
});
