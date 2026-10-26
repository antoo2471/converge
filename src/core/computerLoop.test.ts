import { expect, it } from "vitest";
import { ComputerLoop } from "./computerLoop";
it("stops repeated actions even when their screenshot IDs change", () => {
  const loop = new ComputerLoop();
  for (let i = 0; i < 3; i++)
    expect(
      loop.inspect([
        { name: "computer.click", args: { x: 10, y: 20, frameId: String(i) } },
      ]),
    ).toBeUndefined();
  expect(
    loop.inspect([
      { name: "computer.click", args: { x: 10, y: 20, frameId: "new" } },
    ]),
  ).toContain("même action");
  expect(loop.steps).toBe(3);
});
it("bounds grouped actions without affecting file tools", () => {
  const loop = new ComputerLoop();
  for (let i = 0; i < 2; i++)
    expect(
      loop.inspect([
        {
          name: "computer.actions",
          args: {
            actions: Array.from({ length: 12 }, () => ({
              type: "key",
              key: String(i),
            })),
          },
        },
      ]),
    ).toBeUndefined();
  expect(
    loop.inspect([
      {
        name: "computer.actions",
        args: {
          actions: Array.from({ length: 12 }, () => ({
            type: "key",
            key: "A",
          })),
        },
      },
    ]),
  ).toContain("limite");
  expect(
    loop.inspect([{ name: "filesystem.write", args: { path: "file" } }]),
  ).toBeUndefined();
});
it("stops after consecutive computer failures", () => {
  const loop = new ComputerLoop();
  loop.record(
    [1, 2].map((i) => ({
      id: String(i),
      name: "computer.click",
      args: {},
      status: "failed" as const,
    })),
  );
  expect(loop.inspect([{ name: "computer.capture", args: {} }])).toContain(
    "deux échecs",
  );
});

it("stops alternating observations without useful desktop actions", () => {
  const loop = new ComputerLoop();
  for (const name of ["computer.windows", "computer.focus", "computer.capture"])
    expect(loop.inspect([{ name, args: { windowId: 123 } }])).toBeUndefined();
  expect(
    loop.inspect([{ name: "computer.focus", args: { windowId: 456 } }]),
  ).toContain("sans action utile");
});
it("allows useful desktop actions to restart the observation budget", () => {
  const loop = new ComputerLoop();
  loop.inspect([{ name: "computer.windows", args: {} }]);
  loop.inspect([{ name: "computer.focus", args: { windowId: 123 } }]);
  loop.inspect([{ name: "computer.click", args: { x: 10, y: 20 } }]);
  expect(
    loop.inspect([{ name: "computer.capture", args: {} }]),
  ).toBeUndefined();
});
