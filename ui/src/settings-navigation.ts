export const settingsPages = [
  'devices',
  'speaker',
  'schedule',
  'volume',
  'general',
  'diagnostics',
  'about',
] as const;
export type SettingsPage = (typeof settingsPages)[number];

export function adjacentPage(page: SettingsPage, direction: -1 | 1): SettingsPage | undefined {
  return settingsPages[settingsPages.indexOf(page) + direction];
}
