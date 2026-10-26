// @vitest-environment jsdom
import { afterEach, it, expect, vi } from "vitest";
import { render, fireEvent, screen, cleanup } from "@testing-library/react";
import { GenerativeUI } from "./Renderer";
afterEach(cleanup);
it("keeps metrics, recommendations and chart points in sync with a dashboard control", () => {
  const source = JSON.stringify({
    version: "1.0",
    title: "Performance",
    state: { score: 40 },
    root: {
      type: "stack",
      children: [
        { type: "button", label: "Reset score", action: { type: "reset" } },
        {
          type: "row",
          children: [
            {
              type: "metric",
              label: "Score",
              value: { state: "score" },
              unit: "/100",
              delta: { op: "subtract", args: [{ state: "score" }, 40] },
              tone: "positive",
            },
            { type: "badge", value: "Local" },
          ],
        },
        {
          type: "slider",
          label: "Objectif",
          bind: "score",
          min: 0,
          max: 100,
          unit: "%",
        },
        {
          type: "notice",
          tone: "positive",
          label: "Objectif atteint",
          description: "Le score dépasse le seuil choisi.",
          visible: { op: "gt", args: [{ state: "score" }, 75] },
        },
        {
          type: "chart",
          label: "Évolution",
          variant: "line",
          points: [
            { label: "Départ", value: 40 },
            { label: "Actuel", value: { state: "score" } },
          ],
        },
      ],
    },
  });
  render(<GenerativeUI source={source} onAction={vi.fn()} />);
  expect(screen.queryByText("Performance")).toBeNull();
  expect(screen.queryByText("Interactif")).toBeNull();
  expect(screen.queryByText("Objectif atteint")).toBeNull();
  fireEvent.change(screen.getByRole("slider"), { target: { value: "85" } });
  expect(screen.getByText("Objectif atteint")).toBeTruthy();
  expect(screen.getByText("45")).toBeTruthy();
  expect(
    screen.getByRole("graphics-symbol", { name: "Actuel: 85" }),
  ).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Reset score" }));
  expect(screen.queryByText("Objectif atteint")).toBeNull();
  expect(
    screen.getByRole("graphics-symbol", { name: "Actuel: 40" }),
  ).toBeTruthy();
});

it("plots negative values inside the chart and lets the user hide points", () => {
  const { container } = render(
    <GenerativeUI
      onAction={vi.fn()}
      source={JSON.stringify({
        version: "1.0",
        title: "Balance",
        root: {
          type: "chart",
          points: [
            { label: "Débit", value: -20 },
            { label: "Crédit", value: 40 },
          ],
        },
      })}
    />,
  );
  const bars = Array.from(container.querySelectorAll(".gui-chart svg rect"));
  expect(bars).toHaveLength(2);
  for (const bar of bars) {
    expect(Number(bar.getAttribute("y"))).toBeGreaterThanOrEqual(16);
    expect(
      Number(bar.getAttribute("y")) + Number(bar.getAttribute("height")),
    ).toBeLessThanOrEqual(213);
  }
  fireEvent.click(screen.getByRole("button", { name: "Débit" }));
  expect(
    screen.queryByRole("graphics-symbol", { name: "Débit: -20" }),
  ).toBeNull();
});
it("updates calculated UI immediately without a model request", () => {
  render(
    <GenerativeUI
      source={JSON.stringify({
        version: "1.0",
        title: "Budget",
        state: { amount: 30 },
        root: {
          type: "stack",
          children: [
            {
              type: "slider",
              label: "Montant",
              bind: "amount",
              min: 0,
              max: 100,
            },
            {
              type: "metric",
              label: "Total",
              value: { op: "multiply", args: [{ state: "amount" }, 2] },
            },
          ],
        },
      })}
      onAction={vi.fn()}
    />,
  );
  expect(screen.getByText("60")).toBeTruthy();
  fireEvent.change(screen.getByRole("slider"), { target: { value: "45" } });
  expect(screen.getByText("90")).toBeTruthy();
});
it("filters a real table and forwards only a declared user action", () => {
  const action = vi.fn();
  render(
    <GenerativeUI
      source={JSON.stringify({
        version: "1.0",
        title: "Choix",
        state: { model: "qwen" },
        root: {
          type: "stack",
          children: [
            {
              type: "table",
              columns: ["Modèle"],
              rows: [{ Modèle: "Qwen" }, { Modèle: "Mistral" }],
            },
            {
              type: "button",
              label: "Choisir",
              action: {
                type: "send",
                text: {
                  op: "concat",
                  args: ["Modèle choisi : ", { state: "model" }],
                },
              },
            },
          ],
        },
      })}
      onAction={action}
    />,
  );
  fireEvent.change(
    screen.getByRole("textbox", { name: "Filtrer le tableau" }),
    { target: { value: "Mistral" } },
  );
  expect(screen.queryByText("Qwen")).toBeNull();
  expect(screen.getByText("Mistral")).toBeTruthy();
  expect(action).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Choisir" }));
  expect(action.mock.calls[0][0].type).toBe("send");
  expect(action.mock.calls[0][1].model).toBe("qwen");
});
it("refuses unsupported executable content before rendering", () => {
  render(
    <GenerativeUI
      source={
        '{"version":"1.0","title":"bad","root":{"type":"script","value":"alert(1)"}}'
      }
      onAction={vi.fn()}
    />,
  );
  expect(screen.getByText(/n’a pas pu être affichée/)).toBeTruthy();
  expect(document.querySelector("script")).toBeNull();
});

it("waits until streaming finishes before validating or mounting the interface", () => {
  const onAction = vi.fn();
  const { rerender } = render(
    <GenerativeUI source={'{"version":'} streaming onAction={onAction} />,
  );
  expect(screen.getByRole("status").textContent).toContain("Création");
  expect(screen.queryByText(/n’a pas pu être affichée/)).toBeNull();
  const source = JSON.stringify({
    version: "1.0",
    title: "Greeting",
    state: { name: "" },
    root: {
      type: "stack",
      children: [
        { type: "text", text: "Welcome" },
        {
          type: "input",
          label: "Name",
          placeholder: "Your name",
          bind: "name",
        },
      ],
    },
  });
  rerender(<GenerativeUI source={source} streaming onAction={onAction} />);
  expect(screen.queryByRole("textbox")).toBeNull();
  rerender(
    <GenerativeUI source={source} streaming={false} onAction={onAction} />,
  );
  expect(screen.getByText("Welcome")).toBeTruthy();
  fireEvent.change(screen.getByPlaceholderText("Your name"), {
    target: { value: "Antoo" },
  });
  expect((screen.getByRole("textbox") as HTMLInputElement).value).toBe("Antoo");
});

it("shows a final format failure only after the stream has ended", () => {
  const { rerender } = render(
    <GenerativeUI source="{" streaming onAction={vi.fn()} />,
  );
  expect(screen.queryByText(/n’a pas pu être affichée/)).toBeNull();
  rerender(<GenerativeUI source="{" onAction={vi.fn()} />);
  expect(screen.getByText(/n’a pas pu être affichée/)).toBeTruthy();
  expect(screen.getByText(/JSON est incomplet/)).toBeTruthy();
});
