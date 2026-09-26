// Reject reads that started before a user edit or a newer refresh.
export function canApplyRefresh(
  request: number,
  latest: number,
  revision: number,
  currentRevision: number,
  writing: boolean,
): boolean {
  return request === latest && revision === currentRevision && !writing;
}
