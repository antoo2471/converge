/** An IPC command's return value and its ordered event channel finish independently. */
export async function channelCommand<T, E>(
  start: (deliver: (event: E) => void) => Promise<T>,
  isTerminal: (event: E) => boolean,
  onEvent: (event: E) => void,
  timeoutMs = 30000,
): Promise<T> {
  let closed = false;
  let terminal = false;
  let deliveryError: unknown;
  let finish!: () => void;
  const delivered = new Promise<void>((resolve) => {
    finish = resolve;
  });
  const deliver = (event: E) => {
    if (closed || terminal) return;
    try {
      onEvent(event);
    } catch (error) {
      deliveryError = error;
      finish();
    } finally {
      if (isTerminal(event)) {
        terminal = true;
        finish();
      }
    }
  };
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    const result = await start(deliver);
    await Promise.race([
      delivered,
      new Promise<never>((_, reject) => {
        timer = setTimeout(
          () =>
            reject(
              Error(
                "Le transfert local de la réponse ne s’est pas terminé. Aucun appel incomplet ne sera exécuté.",
              ),
            ),
          timeoutMs,
        );
      }),
    ]);
    if (deliveryError) throw deliveryError;
    return result;
  } finally {
    closed = true;
    if (timer) clearTimeout(timer);
  }
}
