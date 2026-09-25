import type { DesktopPlatform } from './platform';

const icons = {
  speaker:
    '<rect x="6" y="2" width="12" height="20" rx="2"/><circle cx="12" cy="14" r="4"/><circle cx="12" cy="6" r="1"/>',
  output: '<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8m-4-4v4"/>',
  mute: '<path d="m11 4-5 4H3v8h3l5 4V4Zm5 5 6 6m0-6-6 6"/>',
  moon: '<path d="M20 15A9 9 0 0 1 9 4a9 9 0 1 0 11 11Z"/>',
  sound: '<path d="m11 4-5 4H3v8h3l5 4V4Zm4 4a6 6 0 0 1 0 8m3-11a10 10 0 0 1 0 14"/>',
  light:
    '<circle cx="12" cy="12" r="4"/><path d="M12 1v3m0 16v3M1 12h3m16 0h3M4 4l2 2m12 12 2 2M4 20l2-2M18 6l2-2"/>',
  speech:
    '<path d="M21 11a8 8 0 0 1-8 8H8l-5 3V6a3 3 0 0 1 3-3h7a8 8 0 0 1 8 8Z"/><path d="M7 9h10M7 13h7"/>',
  tone: '<path d="M4 3v18M12 3v18M20 3v18"/><path d="M1 8h6m2 8h6m2-10h6"/>',
  sync: '<path d="M20 7A9 9 0 0 0 4 6L2 9m0-6v6h6m-4 8a9 9 0 0 0 16 1l2-3m0 6v-6h-6"/>',
  power: '<path d="M12 2v10M6 5a9 9 0 1 0 12 0"/>',
} as const;

function escapeText(text: string): string {
  return text.replace(
    /[&<>"']/g,
    (character) =>
      ({
        '&': '&amp;',
        '<': '&lt;',
        '>': '&gt;',
        '"': '&quot;',
        "'": '&#39;',
      })[character]!,
  );
}

/** Decorative Windows captions leave the underlying form controls untouched. */
export function settingCaption(
  platform: DesktopPlatform,
  title: string,
  description: string,
  icon: keyof typeof icons,
): string {
  if (platform !== 'windows') return escapeText(title);
  return `<span class="setting-caption"><svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">${icons[icon]}</svg><span>${escapeText(title)}<small>${escapeText(description)}</small></span></span>`;
}
