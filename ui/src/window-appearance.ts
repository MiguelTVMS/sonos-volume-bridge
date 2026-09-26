type FocusSource = {
  listen: (update: (focused: boolean) => void) => Promise<() => void>;
  current: () => Promise<boolean>;
};

// Subscribe before reading initial state, and never let a delayed initial read
// overwrite a newer native focus event.
export async function bindWindowFocus(
  source: FocusSource,
  apply: (focused: boolean) => void,
): Promise<() => void> {
  let receivedEvent = false;
  const unlisten = await source.listen((focused) => {
    receivedEvent = true;
    apply(focused);
  });
  try {
    const focused = await source.current();
    if (!receivedEvent) apply(focused);
  } catch {
    // Keep the event listener: subsequent native events can still restore state.
  }
  return unlisten;
}
