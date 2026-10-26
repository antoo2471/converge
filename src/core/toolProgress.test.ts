import { expect, it } from "vitest";
import {
  preparingArguments,
  preparedLines,
  preparingName,
} from "./toolProgress";
it("counts real generated lines while JSON arguments are still incomplete", () => {
  const source =
    '{"name":"filesystem.write","args":{"path":"site/index.html","content":"one\\ntwo\\nthird';
  expect(preparingName(source)).toBe("filesystem.write");
  expect(preparingArguments(source)).toEqual({
    path: "site/index.html",
    content: "one\ntwo\nthird",
  });
  expect(preparedLines(preparingArguments(source))).toBe(3);
  expect(preparedLines({ content: "one\ntwo\n" })).toBe(2);
});
it("does not throw or fabricate characters from unfinished escapes", () => {
  expect(preparingArguments('{"content":"hello\\u00')).toEqual({
    content: "hello",
  });
  expect(preparingArguments('{"content":"hello\\')).toEqual({
    content: "hello",
  });
});
