export interface CloseRequestEvent {
  preventDefault(): void;
}

export async function handleCloseRequested(
  event: CloseRequestEvent,
  flushOnClose: boolean,
  flushSettings: () => Promise<unknown>,
  flushPendingProgress: () => Promise<unknown>,
  destroy: () => Promise<unknown>,
  reportError: (error: unknown) => void = (error) => {
    console.error("[ioun] No se pudo completar el flush al cerrar", error);
  },
): Promise<void> {
  if (!flushOnClose) return;
  event.preventDefault();
  try {
    await Promise.all([flushSettings(), flushPendingProgress()]);
  } catch (error) {
    reportError(error);
  } finally {
    await destroy();
  }
}
