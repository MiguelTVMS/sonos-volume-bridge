export type DiagnosticsDisclosureState = {
  visible: boolean;
  shouldRefresh: boolean;
};

export function diagnosticsDisclosureState(open: boolean): DiagnosticsDisclosureState {
  return {
    visible: open,
    shouldRefresh: open,
  };
}
