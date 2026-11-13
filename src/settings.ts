import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './theme.css';
import './settings.css';
import { AppStateDto, SHADOW_ROOM, initTheme } from './common';

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

window.addEventListener('DOMContentLoaded', async () => {
  // Register before any await: the window is reused (hidden, not destroyed),
  // and a settings-show emitted during initTheme must not be missed (same
  // race as panel-show in main.ts).
  await listen('settings-show', () => {
    backfill().catch(() => undefined);
  });

  try {
    await initTheme();
  } catch {
    /* backend unavailable: keep default theme */
  }
  await backfill();
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
