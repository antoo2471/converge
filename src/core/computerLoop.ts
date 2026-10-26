import type { ToolCall } from "../types";
type Request = { name: string; args: Record<string, unknown> };
function signature(request: Request) {
  const { frameId: _frame, requestId: _request, ...args } = request.args;
  return JSON.stringify({ name: request.name, args }, (key, value) =>
    key === "frameId" ? undefined : value,
  );
}
/** Bounded autonomy per user turn; never execute an endlessly repeated desktop action. */
export class ComputerLoop {
  rounds = 0;
  steps = 0;
  private failures = 0;
  private observationRounds = 0;
  private repetitions = new Map<string, number>();
  inspect(requests: Request[]): string | undefined {
    const computer = requests.filter((r) => r.name.startsWith("computer."));
    if (!computer.length) return;
    if (this.failures >= 2)
      return "Contrôle arrêté après deux échecs successifs. Vérifiez la fenêtre cible avant de continuer.";
    const acts = computer.some((r) =>
      r.name === "computer.actions"
        ? Array.isArray(r.args.actions) &&
          r.args.actions.some((a: unknown) =>
            ["click", "double_click", "drag", "type", "key", "scroll"].includes(
              String((a as { type?: unknown })?.type),
            ),
          )
        : [
            "computer.click",
            "computer.double_click",
            "computer.drag",
            "computer.type",
            "computer.key",
            "computer.scroll",
          ].includes(r.name),
    );
    const count = computer.reduce(
      (sum, r) =>
        sum +
        (r.name === "computer.actions" && Array.isArray(r.args.actions)
          ? r.args.actions.length
          : 1),
      0,
    );
    if (this.rounds >= 12 || this.steps + count > 32)
      return "Contrôle arrêté à la limite de ce tour pour éviter de poursuivre indéfiniment. Vous pouvez demander de continuer.";
    const pending = new Map(this.repetitions);
    for (const request of computer) {
      const key = signature(request);
      const repeats = (pending.get(key) || 0) + 1;
      if (repeats > 3)
        return "Contrôle arrêté : l’IA répétait la même action. Aucune nouvelle action n’a été envoyée.";
      pending.set(key, repeats);
    }
    if (!acts && this.observationRounds >= 3)
      return "Contrôle arrêté : trois observations sans action utile. L’IA doit agir sur la fenêtre visible ou expliquer ce qui bloque, pas continuer à capturer ou changer de fenêtre.";
    this.observationRounds = acts ? 0 : this.observationRounds + 1;
    this.repetitions = pending;
    this.rounds++;
    this.steps += count;
  }
  record(results: ToolCall[]) {
    for (const call of results.filter((r) => r.name.startsWith("computer."))) {
      this.failures = call.status === "failed" ? this.failures + 1 : 0;
    }
  }
}
