import { invoke as nativeInvoke } from '@tauri-apps/api/core';
import { createCommandBridge, type Invoke } from './command-bridge';
import type { DesktopPlatform } from './platform';

export let invoke: Invoke = nativeInvoke;
export let demoMode = false;
export let presentationOverride: DesktopPlatform | null = null;

export async function initializeCommands(): Promise<void> {
  const bridge = await createCommandBridge(nativeInvoke);
  invoke = bridge.invoke;
  demoMode = bridge.demo;
  if (demoMode)
    presentationOverride = await nativeInvoke<DesktopPlatform | null>('ui_demo_platform');
}
