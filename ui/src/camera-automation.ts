export type CameraAutomationStatus = {
  available: boolean;
  message: string;
  warning?: string | null;
};

export function cameraAutomationPresentation(
  status: CameraAutomationStatus,
  enabled: boolean,
  speechAvailable: boolean,
): { disabled: boolean; message: string } {
  return {
    disabled: !enabled && (!status.available || !speechAvailable),
    message: status.warning ? `${status.message}. ${status.warning}` : status.message,
  };
}
