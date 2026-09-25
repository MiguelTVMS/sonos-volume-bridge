// The hidden label sizes the wrapper; the native select retains focus and menus.
export function bindSelectedLabel(
  select: Pick<HTMLSelectElement, 'selectedOptions' | 'addEventListener'>,
  label: Pick<HTMLElement, 'textContent'>,
): void {
  const update = (): void => {
    label.textContent = select.selectedOptions[0]?.label ?? '';
  };
  update();
  select.addEventListener('change', update);
}

export function sizeSelectedControls(root: HTMLElement): void {
  root.querySelectorAll('select').forEach((select) => {
    const wrapper = document.createElement('span');
    wrapper.className = 'selected-control';
    const label = document.createElement('span');
    label.className = 'selected-control-label';
    label.setAttribute('aria-hidden', 'true');
    select.replaceWith(wrapper);
    wrapper.append(label, select);
    bindSelectedLabel(select, label);
  });
}
