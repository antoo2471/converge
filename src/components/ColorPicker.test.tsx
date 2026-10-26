// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import { ColorPicker } from "./ColorPicker";
afterEach(cleanup);
it("changes hue using an integrated picker and accepts a HEX value", () => {
  const changed = vi.fn();
  function Harness() {
    const [value, setValue] = useState("#ff0000");
    return (
      <ColorPicker
        label="Halo"
        value={value}
        automatic={false}
        onChange={(v) => {
          setValue(v);
          changed(v);
        }}
      />
    );
  }
  const { container } = render(<Harness />);
  fireEvent.click(screen.getByRole("button"));
  expect(container.querySelector('input[type="color"]')).toBeNull();
  fireEvent.change(screen.getByRole("slider", { name: "Halo : Teinte" }), {
    target: { value: "120" },
  });
  expect(changed).toHaveBeenLastCalledWith("#00ff00");
  fireEvent.change(screen.getByRole("textbox"), {
    target: { value: "#123456" },
  });
  expect(changed).toHaveBeenLastCalledWith("#123456");
  fireEvent.change(screen.getByRole("textbox"), { target: { value: "#12" } });
  fireEvent.blur(screen.getByRole("textbox"));
  expect((screen.getByRole("textbox") as HTMLInputElement).value).toBe(
    "#123456",
  );
});
