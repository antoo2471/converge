import { describe, it, expect } from "vitest";
import { parseUI, evaluate, UI_INSTRUCTIONS } from "./schema";
describe("Converge UI trust boundary", () => {
  it("keeps the model's complete example in sync with the validator", () => {
    const example = /```converge-ui\n([\s\S]*?)\n```/.exec(UI_INSTRUCTIONS)![1];
    expect(parseUI(JSON.parse(example)).root.type).toBe("stack");
  });
  it("accepts text and placeholder without accepting arbitrary HTML properties", () => {
    const doc = parseUI({
      version: "1.0",
      title: "Form",
      root: {
        type: "stack",
        children: [
          { type: "text", text: "Hello" },
          { type: "input", placeholder: "Name", bind: "name" },
        ],
      },
    });
    expect(doc.root.children![0].text).toBe("Hello");
    expect(doc.root.children![1].placeholder).toBe("Name");
    expect(() =>
      parseUI({
        version: "1.0",
        title: "Bad",
        root: { type: "input", placeholder: "Name", onClick: "execute()" },
      }),
    ).toThrow();
  });
  it("validates an interactive calculator and evaluates state changes", () => {
    const doc = parseUI({
      version: "1.0",
      title: "Calcul",
      state: { amount: 30 },
      root: {
        type: "stack",
        children: [
          { type: "slider", bind: "amount", min: 0, max: 100 },
          {
            type: "metric",
            value: { op: "multiply", args: [{ state: "amount" }, 2] },
          },
        ],
      },
    });
    const expression = doc.root.children![1].value;
    expect(evaluate(expression, doc.state)).toBe(60);
    expect(evaluate(expression, { amount: 45 })).toBe(90);
  });
  it("rejects scripts, unknown components, action escalation and prototype keys", () => {
    for (const root of [
      { type: "script", value: "alert(1)" },
      { type: "button", action: { type: "execute", command: "whoami" } },
      { type: "input", bind: "constructor" },
      { type: "text", dangerouslySetInnerHTML: "x" },
    ]) {
      expect(() =>
        parseUI({ version: "1.0", title: "Bad", state: {}, root }),
      ).toThrow();
    }
  });
  it("rejects hostile recursive documents before schema traversal", () => {
    let root: unknown = { type: "text", value: "x" };
    for (let i = 0; i < 100; i++) root = { type: "stack", children: [root] };
    expect(() =>
      parseUI({ version: "1.0", title: "Bad", state: {}, root }),
    ).toThrow();
  });
  it("does not evaluate code or non-finite arithmetic", () => {
    expect(evaluate({ op: "divide", args: [1, 0] }, {})).toBeNull();
    expect(evaluate("window.alert(1)", {})).toBe("window.alert(1)");
    expect(evaluate({ state: "constructor" }, {})).toBeNull();
  });
  it("rejects unrecognized schema versions", () => {
    expect(() =>
      parseUI({
        version: "2.0",
        title: "x",
        state: {},
        root: { type: "text" },
      }),
    ).toThrow();
  });
});
