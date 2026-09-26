export type SpeakerFeature =
  'loudness' | 'nightSound' | 'speechEnhancement' | 'statusLight' | 'treble' | 'bass';
export type FeatureAvailability = 'supported' | 'unsupported' | 'unavailable';
export type SpeakerSettings = {
  loudness: boolean | null;
  nightSound: boolean | null;
  speechEnhancement: boolean | null;
  statusLight: boolean | null;
  treble: number | null;
  bass: number | null;
  capabilities?: Partial<Record<SpeakerFeature, FeatureAvailability>>;
};

// Shared by first render and push refreshes: only speaker controls are touched.
export function applySpeakerControls(
  scope: ParentNode,
  settings: SpeakerSettings,
  activeElement: Element | null,
): void {
  scope
    .querySelectorAll<HTMLInputElement>('[data-speaker-setting], [data-speaker-level]')
    .forEach((input) => {
      const feature = (input.dataset.speakerSetting ??
        input.dataset.speakerLevel) as SpeakerFeature;
      const value = settings[feature];
      const availability =
        settings.capabilities?.[feature] ?? (value === null ? 'unavailable' : 'supported');
      const message =
        availability === 'unsupported'
          ? 'Not supported by this speaker'
          : value === null
            ? 'Temporarily unavailable'
            : '';
      input.disabled = availability !== 'supported' || value === null;
      input.title = message;
      const status = scope.querySelector<HTMLElement>(`[data-feature-status="${feature}"]`);
      if (status) status.textContent = message;
      if (input.dataset.speakerSetting) input.checked = value === true && !input.disabled;
      else if (input !== activeElement) {
        input.value = String(value ?? 0);
        const output = input.closest('label')?.querySelector('output');
        if (output) output.value = String(value ?? 'Unavailable');
      }
    });
}
