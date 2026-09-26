export type ScheduleNotifications = 'never' | 'start' | 'end' | 'both';
export type NightSchedule = { enabled: boolean; blocks: boolean[][] };
export type ScheduleStatus = {
  active: boolean;
  supported: boolean;
  message: string;
  nextTransition: string | null;
  timeZone: string;
  notificationsBlocked: boolean;
};
export const lockMessage =
  'Night Mode is on because of your schedule. Disable the schedule to turn Night Mode off.';
const days = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
export const emptyBlocks = (): boolean[][] =>
  Array.from({ length: 7 }, () => Array<boolean>(48).fill(false));
const copy = (blocks: boolean[][]): boolean[][] => blocks.map((day) => [...day]);
let systemHour12: boolean | undefined;
export function setSystemHour12(value: boolean | null): void {
  systemHour12 = value ?? undefined;
}
export function timeLabel(slot: number, locale?: string, hourOnly = false): string {
  const format = new Intl.DateTimeFormat(locale, {
    hour: hourOnly ? 'numeric' : '2-digit',
    ...(systemHour12 === undefined ? {} : { hour12: systemHour12 }),
    ...(hourOnly ? {} : { minute: '2-digit' as const }),
  });
  if (slot === 48 && !format.resolvedOptions().hour12) return hourOnly ? '24' : '24:00';
  return format.format(new Date(2020, 0, 1, 0, slot * 30));
}
export class ScheduleDraft {
  blocks = emptyBlocks();
  dirty = false;
  revision = 0;
  painting: boolean | null = null;
  private anchor: { day: number; slot: number; blocks: boolean[][] } | null = null;
  refresh(saved: boolean[][]): void {
    if (!this.dirty) this.blocks = copy(saved);
  }
  paint(day: number, slot: number): void {
    if (!this.anchor) {
      this.anchor = { day, slot, blocks: copy(this.blocks) };
      this.painting = !this.blocks[day][slot];
    }
    this.blocks = copy(this.anchor.blocks);
    for (let row = Math.min(day, this.anchor.day); row <= Math.max(day, this.anchor.day); row++) {
      for (
        let column = Math.min(slot, this.anchor.slot);
        column <= Math.max(slot, this.anchor.slot);
        column++
      ) {
        this.blocks[row][column] = this.painting!;
      }
    }
    this.dirty = true;
    this.revision++;
  }
  end(): void {
    this.painting = null;
    this.anchor = null;
  }
  clear(): void {
    this.blocks = emptyBlocks();
    this.dirty = true;
    this.revision++;
  }
  reset(saved: boolean[][]): void {
    this.dirty = false;
    this.revision++;
    this.end();
    this.refresh(saved);
  }
}
export const scheduleDraft = new ScheduleDraft();
let savedScroll = 0;
let savedFocus: string | null = null;
export function captureScheduleView(scope: ParentNode): void {
  const scroll = scope.querySelector<HTMLElement>('.schedule-scroll');
  if (scroll) savedScroll = scroll.scrollTop;
  const focused = document.activeElement as HTMLElement | null;
  savedFocus = focused?.classList.contains('schedule-cell')
    ? `[data-day="${focused.dataset.day}"][data-slot="${focused.dataset.slot}"]`
    : null;
}

export function scheduleMarkup(): string {
  return `<section id="night-schedule" data-schedule class="night-schedule" aria-labelledby="night-schedule-title">
    <div class="settings-group">
    <label class="toggle"><span>Enable schedule</span><input id="schedule-enabled" type="checkbox" role="switch"></label>
    <div class="control-field"><label for="schedule-notifications">Night schedule notifications</label><select id="schedule-notifications"><option value="start">On start</option><option value="end">On end</option><option value="both">On start and end</option><option value="never">Never</option></select></div>
    <p id="notification-permission" class="setting-note" aria-live="polite"></p>
    </div>
    <p id="schedule-status" class="setting-note" aria-live="polite"></p>
    <div class="settings-group schedule-editor"><div class="schedule-editor-content">
    <p class="schedule-legend"><span>■ Scheduled on</span> □ Manual control</p>
    <div class="schedule-scroll"><div class="schedule-grid" role="grid" aria-label="Weekly Night Mode schedule"></div></div>
    <p class="setting-note">Select 30-minute blocks. Drag to select a rectangle; arrow keys move and Space selects. Leaving a scheduled period turns Night Mode off. Times follow your computer's time zone.</p>
    <p class="setting-note">Save schedule applies the current block immediately, even when scheduling is disabled. With notifications enabled, each save also tests the native notification.</p><div class="schedule-actions"><button type="button" id="schedule-save">Save schedule</button><button type="button" class="secondary" id="schedule-cancel">Cancel</button><button type="button" class="secondary" id="schedule-clear">Clear all</button></div>
    </div></div>
  </section>`;
}
type Actions = {
  save: (blocks: boolean[][]) => Promise<void>;
  enable: (enabled: boolean) => Promise<void>;
  notify: (mode: ScheduleNotifications) => Promise<void>;
  error: (message: string) => void;
};
export function mountSchedule(
  scope: ParentNode,
  saved: NightSchedule,
  notifications: ScheduleNotifications,
  actions: Actions,
): void {
  const panel = scope.querySelector<HTMLElement>('#night-schedule');
  if (!panel) return;
  scheduleDraft.refresh(saved.blocks);
  const grid = panel.querySelector<HTMLElement>('.schedule-grid')!;
  grid.innerHTML =
    `<div class="schedule-head" role="row"><span></span><div class="schedule-hours">${[0, 3, 6, 9, 12, 15, 18, 21].map((hour) => `<span role="columnheader">${timeLabel(hour * 2, undefined, true)}</span>`).join('')}</div></div>` +
    days
      .map(
        (day, index) =>
          `<div class="schedule-row" role="row"><span role="rowheader">${day}</span>${Array.from({ length: 48 }, (_, slot) => `<button type="button" role="gridcell" class="schedule-cell" data-day="${index}" data-slot="${slot}" data-tooltip="${day} ${timeLabel(slot)}–${timeLabel(slot + 1)}" aria-label="${day} ${timeLabel(slot)}–${timeLabel(slot + 1)}" tabindex="${slot === 0 && index === 0 ? 0 : -1}"></button>`).join('')}</div>`,
      )
      .join('');
  const tooltip = document.createElement('div');
  tooltip.className = 'schedule-tooltip';
  tooltip.setAttribute('role', 'tooltip');
  tooltip.hidden = true;
  panel.append(tooltip);
  let tooltipTimer: ReturnType<typeof setTimeout> | undefined;
  const hideTooltip = (): void => {
    clearTimeout(tooltipTimer);
    tooltip.hidden = true;
  };
  grid.addEventListener('pointerover', (event) => {
    hideTooltip();
    const cell = (event.target as Element).closest<HTMLButtonElement>('.schedule-cell');
    if (!cell) return;
    tooltipTimer = setTimeout(() => {
      if (!cell.isConnected || scheduleDraft.painting !== null) return;
      const rect = cell.getBoundingClientRect();
      tooltip.textContent = cell.dataset.tooltip ?? '';
      tooltip.hidden = false;
      tooltip.style.left = `${Math.max(8, Math.min(rect.left, window.innerWidth - tooltip.offsetWidth - 8))}px`;
      tooltip.style.top = `${Math.max(8, rect.top - tooltip.offsetHeight - 6)}px`;
    }, 100);
  });
  grid.addEventListener('pointerout', hideTooltip);
  grid.addEventListener('pointerdown', hideTooltip);
  grid.addEventListener('scroll', hideTooltip, true);
  const enabled = panel.querySelector<HTMLInputElement>('#schedule-enabled')!;
  const notify = panel.querySelector<HTMLSelectElement>('#schedule-notifications')!;
  enabled.checked = saved.enabled;
  notify.value = notifications;
  const repaint = (): void => {
    grid.querySelectorAll<HTMLButtonElement>('.schedule-cell').forEach((cell) => {
      cell.setAttribute(
        'aria-selected',
        String(scheduleDraft.blocks[Number(cell.dataset.day)][Number(cell.dataset.slot)]),
      );
    });
    panel.querySelector<HTMLButtonElement>('#schedule-save')!.disabled =
      grid.dataset.supported !== 'true';
    panel.querySelector<HTMLButtonElement>('#schedule-cancel')!.disabled = !scheduleDraft.dirty;
  };
  const cellAt = (x: number, y: number): HTMLButtonElement | null =>
    document.elementFromPoint(x, y)?.closest<HTMLButtonElement>('.schedule-cell') ?? null;
  let pointer: number | null = null;
  let position = { x: 0, y: 0 };
  let frame = 0;
  const paint = (cell: HTMLButtonElement | null): void => {
    if (!cell || !grid.contains(cell) || cell.disabled) return;
    scheduleDraft.paint(Number(cell.dataset.day), Number(cell.dataset.slot));
    repaint();
  };
  const scroll = panel.querySelector<HTMLElement>('.schedule-scroll')!;
  const animate = (): void => {
    if (pointer === null || !grid.isConnected) return;
    const rect = scroll.getBoundingClientRect();
    if (position.y > rect.bottom - 25) scroll.scrollTop += 8;
    if (position.y < rect.top + 45) scroll.scrollTop -= 8;
    paint(cellAt(position.x, position.y));
    frame = requestAnimationFrame(animate);
  };
  grid.addEventListener('pointerdown', (event) => {
    if (event.button !== 0 || grid.dataset.supported !== 'true') return;
    const cell = (event.target as Element).closest<HTMLButtonElement>('.schedule-cell');
    if (!cell) return;
    event.preventDefault();
    cell.focus();
    pointer = event.pointerId;
    position = { x: event.clientX, y: event.clientY };
    grid.setPointerCapture(pointer);
    paint(cell);
    frame = requestAnimationFrame(animate);
  });
  grid.addEventListener('pointermove', (event) => {
    if (pointer !== event.pointerId) return;
    position = { x: event.clientX, y: event.clientY };
    paint(cellAt(position.x, position.y));
  });
  const end = (): void => {
    pointer = null;
    scheduleDraft.end();
    cancelAnimationFrame(frame);
  };
  grid.addEventListener('pointerup', end);
  grid.addEventListener('pointercancel', end);
  grid.addEventListener('lostpointercapture', end);
  grid.addEventListener('click', (event) => {
    if (event.detail !== 0) return; // Pointer gestures already paint; assistive clicks toggle.
    paint((event.target as Element).closest<HTMLButtonElement>('.schedule-cell'));
    scheduleDraft.end();
  });
  grid.addEventListener('keydown', (event) => {
    const cell = (event.target as Element).closest<HTMLButtonElement>('.schedule-cell');
    if (!cell) return;
    let day = Number(cell.dataset.day);
    let slot = Number(cell.dataset.slot);
    if (event.key === 'ArrowLeft') slot = Math.max(0, slot - 1);
    else if (event.key === 'ArrowRight') slot = Math.min(47, slot + 1);
    else if (event.key === 'ArrowUp') day = Math.max(0, day - 1);
    else if (event.key === 'ArrowDown') day = Math.min(6, day + 1);
    else return;
    event.preventDefault();
    cell.tabIndex = -1;
    const next = grid.querySelector<HTMLButtonElement>(`[data-day="${day}"][data-slot="${slot}"]`)!;
    next.tabIndex = 0;
    next.focus();
  });
  const run = (operation: () => Promise<void>): void => {
    void operation().catch((error: unknown) => actions.error(String(error)));
  };
  enabled.addEventListener('change', () => run(() => actions.enable(enabled.checked)));
  notify.addEventListener('change', () =>
    run(() => actions.notify(notify.value as ScheduleNotifications)),
  );
  panel
    .querySelector('#schedule-save')!
    .addEventListener('click', () => run(() => actions.save(copy(scheduleDraft.blocks))));
  panel.querySelector('#schedule-cancel')!.addEventListener('click', () => {
    scheduleDraft.reset(saved.blocks);
    repaint();
  });
  panel.querySelector('#schedule-clear')!.addEventListener('click', () => {
    scheduleDraft.clear();
    repaint();
  });
  repaint();
  scroll.scrollTop = savedScroll;
  if (savedFocus) {
    grid.querySelectorAll<HTMLButtonElement>('.schedule-cell').forEach((cell) => {
      cell.tabIndex = -1;
    });
    const focused = grid.querySelector<HTMLButtonElement>(savedFocus);
    if (focused) {
      focused.tabIndex = 0;
      focused.focus({ preventScroll: true });
    }
  }
}
export function updateScheduleView(
  scope: ParentNode,
  status: ScheduleStatus,
  supported: boolean,
  enabled: boolean,
): void {
  const panel = scope.querySelector<HTMLElement>('#night-schedule');
  if (!panel) return;
  panel.querySelector('#schedule-status')!.textContent = [
    ['Schedule disabled.', 'Outside scheduled hours. Manual control is available.'].includes(
      status.message,
    )
      ? ''
      : status.message,
    status.nextTransition ? `Next change: ${status.nextTransition}` : '',
  ]
    .filter(Boolean)
    .join(' · ');
  panel.querySelector('#notification-permission')!.textContent = status.notificationsBlocked
    ? 'Notifications are blocked. Allow Sonos Volume Bridge notifications in system settings.'
    : '';
  panel.querySelector<HTMLElement>('.schedule-grid')!.dataset.supported = String(supported);
  panel.querySelectorAll<HTMLButtonElement>('.schedule-cell, #schedule-clear').forEach((cell) => {
    cell.disabled = !supported;
  });
  panel.querySelector<HTMLInputElement>('#schedule-enabled')!.disabled = !enabled && !supported;
  panel.querySelector<HTMLButtonElement>('#schedule-save')!.disabled = !supported;
  const toggle = scope.querySelector<HTMLInputElement>('[data-speaker-setting="nightSound"]');
  const label = scope.querySelector<HTMLElement>('[data-feature-status="nightSound"]');
  if (toggle && enabled && status.active && supported) {
    toggle.disabled = true;
    toggle.title = lockMessage;
    if (label) label.textContent = lockMessage;
  }
}
