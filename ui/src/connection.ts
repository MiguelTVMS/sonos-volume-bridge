const connectedStatuses = new Set([
  'synchronized',
  'waitingForSonosConfirmation',
  'subscriptionDegraded',
  'pollingFallback',
]);

export type ConnectionLabel = 'Connected' | 'Disconnected';

export function connectionLabel(status: string): ConnectionLabel {
  return connectedStatuses.has(status) ? 'Connected' : 'Disconnected';
}
