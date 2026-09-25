// Hold draft slider values until the gesture ends, including pauses while held.
export class SliderInteraction {
  private editing = new Set<HTMLInputElement>();
  get active(): boolean {
    return this.editing.size > 0;
  }
  bind(input: HTMLInputElement, edited: () => void, preview: () => void, commit: () => void): void {
    let held = false;
    let dirty = false;
    const begin = (): void => {
      held = true;
      this.editing.add(input);
      edited();
    };
    const finish = (): void => {
      held = false;
      this.editing.delete(input);
      if (dirty) {
        dirty = false;
        commit();
      }
    };
    input.addEventListener('pointerdown', (event) => {
      begin();
      input.setPointerCapture(event.pointerId);
    });
    input.addEventListener('keydown', (event) => {
      if (
        [
          'ArrowLeft',
          'ArrowRight',
          'ArrowUp',
          'ArrowDown',
          'Home',
          'End',
          'PageUp',
          'PageDown',
        ].includes(event.key)
      )
        begin();
    });
    input.addEventListener('input', () => {
      dirty = true;
      edited();
      preview();
    });
    input.addEventListener('change', () => {
      if (!held) finish();
    });
    input.addEventListener('pointerup', finish);
    input.addEventListener('pointercancel', finish);
    input.addEventListener('lostpointercapture', finish);
    input.addEventListener('keyup', finish);
    input.addEventListener('blur', finish);
  }
}
