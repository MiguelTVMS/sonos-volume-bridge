export type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

// Select once, before rendering. A failed handshake must never silently enable mocks.
export async function createCommandBridge(nativeInvoke: Invoke, loadDemo: () => Promise<Invoke>) {
  const demo = await nativeInvoke<boolean>('ui_demo_enabled');
  return { demo, invoke: demo ? await loadDemo() : nativeInvoke };
}
