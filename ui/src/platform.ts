export type DesktopPlatform = 'macos' | 'windows' | 'linux' | 'generic';

// Desktop WebViews identify their host OS; keep unknown hosts on the base theme.
export function desktopPlatform(userAgent: string): DesktopPlatform {
  if (/Windows NT/i.test(userAgent)) return 'windows';
  if (/Macintosh|Mac OS X/i.test(userAgent)) return 'macos';
  if (/Linux|X11/i.test(userAgent)) return 'linux';
  return 'generic';
}
