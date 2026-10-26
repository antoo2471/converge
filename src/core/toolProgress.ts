/** Decode a JSON string prefix for display only; never used as executable arguments. */
function stringPrefix(source: string, key: string): string | undefined {
  const match = new RegExp(`"${key}"\\s*:\\s*"`).exec(source);
  if (!match) return undefined;
  let result = "";
  for (let i = match.index + match[0].length; i < source.length; i++) {
    const char = source[i];
    if (char === '"') break;
    if (char !== "\\") {
      result += char;
      continue;
    }
    const next = source[++i];
    if (next === undefined) break;
    if (next === "u") {
      const digits = source.slice(i + 1, i + 5);
      if (!/^[\da-f]{4}$/i.test(digits)) break;
      result += String.fromCharCode(parseInt(digits, 16));
      i += 4;
    } else
      result +=
        (
          {
            n: "\n",
            r: "\r",
            t: "\t",
            b: "\b",
            f: "\f",
            '"': '"',
            "\\": "\\",
            "/": "/",
          } as Record<string, string>
        )[next] ?? "";
  }
  return result;
}
export function preparingArguments(source: string): Record<string, unknown> {
  const args: Record<string, unknown> = {};
  for (const key of [
    "path",
    "content",
    "search",
    "replace",
    "command",
    "cwd",
    "workspace",
  ]) {
    const value = stringPrefix(source, key);
    if (value !== undefined) args[key] = value;
  }
  return args;
}
export function preparedLines(args: Record<string, unknown>) {
  const value =
    typeof args.content === "string"
      ? args.content
      : typeof args.replace === "string"
        ? args.replace
        : "";
  return value ? value.split("\n").length - (value.endsWith("\n") ? 1 : 0) : 0;
}

export function preparingName(source: string) {
  const name = stringPrefix(source, "name");
  return name && /^[a-z]+[._][a-z_]+$/.test(name)
    ? name.replace("_", ".")
    : undefined;
}
