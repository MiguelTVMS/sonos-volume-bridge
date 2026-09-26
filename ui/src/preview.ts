// Development-only entry point. Vite's production build includes index.html only.
// All commands are mocked: this preview cannot discover or modify real devices.
import { mockIPC } from '@tauri-apps/api/mocks';
import { createDemoBackend } from './demo-backend';

const platform = new URLSearchParams(location.search).get('platform') ?? 'macos';
const agents: Record<string, string> = {
  macos: 'Macintosh; Intel Mac OS X',
  windows: 'Windows NT 10.0',
  linux: 'X11; Linux x86_64',
};
Object.defineProperty(navigator, 'userAgent', { value: agents[platform] ?? 'Preview' });
const backend = createDemoBackend({
  hour12: new URLSearchParams(location.search).get('hour12') === 'false' ? false : null,
});
mockIPC((command, args) => {
  if (command === 'ui_demo_enabled') return true;
  if (command === 'ui_demo_platform') return new URLSearchParams(location.search).get('ui');
  if (command === 'plugin:app|version') return 'Preview';
  return backend(command, args as Record<string, unknown>);
});
const { settingsReady } = await import('./bootstrap');
await settingsReady;

// Optional appearance overrides affect only this isolated preview document.
const appearance = new URLSearchParams(location.search).get('appearance');
if (appearance === 'light' || appearance === 'dark') {
  for (const sheet of document.styleSheets) {
    for (const rule of sheet.cssRules) {
      if (rule instanceof CSSMediaRule && rule.conditionText === '(prefers-color-scheme: dark)') {
        rule.media.mediaText = appearance === 'dark' ? 'all' : 'not all';
      }
    }
  }
  document.documentElement.style.colorScheme = appearance;
}
