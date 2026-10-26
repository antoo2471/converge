// @vitest-environment jsdom
import { useState } from "react";
import {
  render,
  screen,
  fireEvent,
  cleanup,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ConversationWorkspaces } from "./ConversationWorkspaces";
import * as api from "../lib/native";
vi.mock("../lib/native", () => ({
  workspaceContext: vi.fn().mockResolvedValue({ folders: [] }),
  chooseWorkspace: vi.fn().mockResolvedValue(["C:\\site", "C:\\assets"]),
  removeWorkspace: vi.fn().mockResolvedValue(["C:\\assets"]),
}));
afterEach(cleanup);
it("adds multiple folders to the intended conversation and removes only its association", async () => {
  function Harness() {
    const [folders, setFolders] = useState<string[]>([]);
    return (
      <ConversationWorkspaces
        conversationId="conversation-a"
        folders={folders}
        onChange={setFolders}
        onClose={vi.fn()}
        onError={vi.fn()}
      />
    );
  }
  render(<Harness />);
  await waitFor(() =>
    expect(api.workspaceContext).toHaveBeenCalledWith("conversation-a"),
  );
  fireEvent.click(screen.getByRole("button", { name: "Ajouter un dossier" }));
  await screen.findByText("C:\\site");
  expect(screen.getByText("C:\\assets")).toBeTruthy();
  expect(api.chooseWorkspace).toHaveBeenCalledWith("conversation-a");
  fireEvent.click(screen.getByRole("button", { name: "Retirer C:\\site" }));
  await waitFor(() => expect(screen.queryByText("C:\\site")).toBeNull());
  expect(api.removeWorkspace).toHaveBeenCalledWith(
    "conversation-a",
    "C:\\site",
  );
});
