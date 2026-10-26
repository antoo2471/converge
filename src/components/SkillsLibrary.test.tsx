// @vitest-environment jsdom
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { SkillsLibrary } from "./SkillsLibrary";
import * as api from "../lib/extensions";
vi.mock("../lib/native", () => ({ native: true, openExternal: vi.fn() }));
vi.mock("../lib/extensions", () => ({
  skillCatalogue: vi.fn(),
  installSkill: vi.fn(),
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
it("uses the skills catalogue and installs the selected source and skill only", async () => {
  vi.mocked(api.skillCatalogue).mockResolvedValue({
    skills: [
      {
        id: "owner/repo/skill",
        name: "skill",
        skillId: "skill",
        source: "owner/repo",
        installs: 123,
      },
    ],
  });
  vi.mocked(api.installSkill).mockResolvedValue({
    id: "installed",
    name: "skill",
    description: "Skill",
    enabled: true,
    subdirectory: "",
  });
  const refresh = vi.fn(async () => {});
  render(
    <SkillsLibrary query="typescript" installed={[]} onInstalled={refresh} />,
  );
  await screen.findByRole("heading", { name: "skill" });
  expect(api.skillCatalogue).toHaveBeenCalledWith("typescript");
  fireEvent.click(screen.getByRole("button", { name: "Installer skill" }));
  await waitFor(() =>
    expect(api.installSkill).toHaveBeenCalledWith("owner/repo", "skill"),
  );
  expect(refresh).toHaveBeenCalled();
});
