// @vitest-environment jsdom
import {
  render,
  screen,
  fireEvent,
  cleanup,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ToolActivity } from "./ToolActivity";
import type { ToolCall, ToolPermission } from "../types";
afterEach(cleanup);
const call: ToolCall = {
  id: "call",
  name: "filesystem.write",
  args: { path: "hello.txt", content: "hello" },
  status: "awaiting",
};
const permission: ToolPermission = {
  id: "permission",
  conversationId: "conversation",
  name: call.name,
  args: call.args,
  root: "C:\\Desktop",
  requiresRoot: true,
};
it("offers real permission choices without a manual execute step", async () => {
  const onDecision = vi.fn().mockResolvedValue(undefined);
  render(
    <ToolActivity
      call={call}
      permission={permission}
      onDecision={onDecision}
      onError={vi.fn()}
    />,
  );
  expect(screen.queryByText(/Examiner et exécuter/)).toBeNull();
  expect(onDecision).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", {
      name: "Tout autoriser dans cette conversation",
    }),
  );
  await waitFor(() =>
    expect(onDecision).toHaveBeenCalledWith(
      permission,
      "conversation",
      "C:\\Desktop",
    ),
  );
});
it("allows refusal even before a folder has been selected", async () => {
  const onDecision = vi.fn().mockResolvedValue(undefined);
  render(
    <ToolActivity
      call={call}
      permission={{ ...permission, root: null }}
      onDecision={onDecision}
      onError={vi.fn()}
    />,
  );
  expect(
    (
      screen.getByRole("button", {
        name: "Autoriser",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Refuser" }));
  await waitFor(() =>
    expect(onDecision).toHaveBeenCalledWith(expect.anything(), "deny", null),
  );
});

it("shows Web sources and computer screenshots without exposing encoded image data", () => {
 const props={onDecision:vi.fn(),onError:vi.fn()};
 const view=render(<ToolActivity {...props} call={{...call,name:"web.search",status:"succeeded",args:{query:"Rust"},result:JSON.stringify({engine:"Bing",results:[{title:"Rust official",url:"https://www.rust-lang.org/"}]})}}/>);
 expect(screen.getByText("Bing · 1 résultat(s)")).toBeTruthy();
 expect(screen.getByRole("button",{name:/Rust official/})).toBeTruthy();
 view.rerender(<ToolActivity {...props} call={{...call,name:"computer.focus",status:"succeeded",args:{windowId:42},result:JSON.stringify({kind:"computer_observation",image:"data:image/jpeg;base64,aGVsbG8=",frameId:"fresh",width:720,height:300})}}/>);
 expect(screen.getByRole("img").getAttribute("src")).toBe("data:image/jpeg;base64,aGVsbG8=");
 expect(view.container.textContent).not.toContain("aGVsbG8=");
});
