// @vitest-environment jsdom
import {
  fireEvent,
  render,
  screen,
  cleanup,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { AppDialogs, useAppDialogs } from "./AppDialogs";
afterEach(cleanup);

function Harness({ onResult }: { onResult: (value: unknown) => void }) {
  const { askText, askConfirm } = useAppDialogs();
  return (
    <>
      <button
        onClick={async () =>
          onResult(await askText("Renommer", "Ancien titre"))
        }
      >
        Nom
      </button>
      <button onClick={async () => onResult(await askConfirm("Supprimer ?"))}>
        Supprimer
      </button>
    </>
  );
}
it("edits a prefilled title in the app", async () => {
  const result = vi.fn();
  render(
    <AppDialogs>
      <Harness onResult={result} />
    </AppDialogs>,
  );
  fireEvent.click(screen.getByText("Nom"));
  fireEvent.change(screen.getByRole("textbox"), {
    target: { value: "  Nouveau titre  " },
  });
  fireEvent.click(screen.getByText("Enregistrer"));
  await waitFor(() => expect(result).toHaveBeenCalledWith("Nouveau titre"));
  expect(screen.queryByRole("dialog")).toBeNull();
});
it("cancels a destructive action with Escape", async () => {
  const result = vi.fn();
  render(
    <AppDialogs>
      <Harness onResult={result} />
    </AppDialogs>,
  );
  fireEvent.click(screen.getByText("Supprimer"));
  expect(screen.getByRole("dialog")).toBeTruthy();
  fireEvent.keyDown(document, { key: "Escape" });
  await waitFor(() => expect(result).toHaveBeenCalledWith(false));
});
