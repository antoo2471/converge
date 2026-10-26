import { z } from "zod";
const key = z
  .string()
  .regex(/^[a-zA-Z][a-zA-Z0-9_]{0,63}$/)
  .refine((k) => !["__proto__", "prototype", "constructor"].includes(k));
const scalar = z.union([
  z.string().max(20000),
  z.number().finite(),
  z.boolean(),
  z.null(),
]);
export type Scalar = z.infer<typeof scalar>;
export type Expr =
  | Scalar
  | { state: string }
  | {
      op:
        | "add"
        | "subtract"
        | "multiply"
        | "divide"
        | "pow"
        | "min"
        | "max"
        | "round"
        | "eq"
        | "gt"
        | "lt"
        | "and"
        | "or"
        | "concat"
        | "if";
      args: Expr[];
    };
export const expression: z.ZodType<Expr> = z.lazy(() =>
  z.union([
    scalar,
    z.object({ state: key }).strict(),
    z
      .object({
        op: z.enum([
          "add",
          "subtract",
          "multiply",
          "divide",
          "pow",
          "min",
          "max",
          "round",
          "eq",
          "gt",
          "lt",
          "and",
          "or",
          "concat",
          "if",
        ]),
        args: z.array(expression).min(1).max(20),
      })
      .strict(),
  ]),
);
const action = z.discriminatedUnion("type", [
  z.object({ type: z.literal("set"), key, value: expression }).strict(),
  z.object({ type: z.literal("reset") }).strict(),
  z.object({ type: z.literal("send"), text: expression }).strict(),
  z
    .object({
      type: z.literal("tool"),
      name: z.string().max(80),
      args: z.record(key, expression),
    })
    .strict(),
  z
    .object({
      type: z.literal("export"),
      filename: z.string().max(100),
      content: expression,
    })
    .strict(),
]);
export type UIAction = z.infer<typeof action>;
export interface UINode {
  type: string;
  id?: string;
  label?: string;
  value?: Expr;
  text?: Expr;
  placeholder?: string;
  description?: string;
  tone?: "neutral" | "positive" | "warning" | "negative";
  unit?: string;
  delta?: Expr;
  bind?: string;
  min?: number;
  max?: number;
  step?: number;
  options?: { label: string; value: Scalar }[];
  children?: UINode[];
  action?: UIAction;
  columns?: string[];
  rows?: Record<string, Expr>[];
  points?: { label: string; value: Expr }[];
  variant?: "line" | "bar";
  visible?: Expr;
  src?: string;
  lat?: Expr;
  lng?: Expr;
  zoom?: number;
}
const node: z.ZodType<UINode> = z.lazy(() =>
  z
    .object({
      type: z.enum([
        "stack",
        "row",
        "text",
        "heading",
        "input",
        "number",
        "textarea",
        "checkbox",
        "radio",
        "select",
        "slider",
        "button",
        "table",
        "chart",
        "metric",
        "divider",
        "progress",
        "tabs",
        "tab",
        "carousel",
        "card",
        "diagram",
        "map",
        "form",
        "badge",
        "notice",
      ]),
      id: key.optional(),
      label: z.string().max(500).optional(),
      value: expression.optional(),
      text: expression.optional(),
      placeholder: z.string().max(500).optional(),
      description: z.string().max(1000).optional(),
      tone: z.enum(["neutral", "positive", "warning", "negative"]).optional(),
      unit: z.string().max(40).optional(),
      delta: expression.optional(),
      bind: key.optional(),
      min: z.number().finite().optional(),
      max: z.number().finite().optional(),
      step: z.number().positive().optional(),
      options: z
        .array(z.object({ label: z.string().max(200), value: scalar }).strict())
        .max(100)
        .optional(),
      children: z.array(node).max(100).optional(),
      action: action.optional(),
      columns: z.array(z.string().max(100)).max(20).optional(),
      rows: z
        .array(z.record(z.string().max(100), expression))
        .max(500)
        .optional(),
      points: z
        .array(
          z.object({ label: z.string().max(100), value: expression }).strict(),
        )
        .max(500)
        .optional(),
      variant: z.enum(["line", "bar"]).optional(),
      visible: expression.optional(),
      src: z.string().max(200000).optional(),
      lat: expression.optional(),
      lng: expression.optional(),
      zoom: z.number().min(1).max(18).optional(),
    })
    .strict(),
);
export const uiSchema = z
  .object({
    version: z.literal("1.0"),
    title: z.string().max(160),
    state: z.record(key, scalar).default({}),
    root: node,
  })
  .strict();
export type UIDocument = z.infer<typeof uiSchema>;
export function parseUI(input: unknown): UIDocument {
  const serialized = JSON.stringify(input);
  if (serialized.length > 500000) throw Error("Interface trop volumineuse");
  let count = 0;
  function check(value: unknown, depth: number) {
    if (depth > 24 || ++count > 12000) throw Error("Interface trop complexe");
    if (value && typeof value === "object") {
      for (const [k, v] of Object.entries(value)) {
        if (["__proto__", "constructor", "prototype"].includes(k))
          throw Error("Clé interdite");
        check(v, depth + 1);
      }
    }
  }
  check(input, 0);
  return uiSchema.parse(input);
}
export function evaluate(
  expr: Expr | undefined,
  state: Record<string, Scalar>,
  depth = 0,
): Scalar {
  if (expr === undefined || depth > 20) return null;
  if (expr === null || typeof expr !== "object") return expr;
  if ("state" in expr)
    return Object.hasOwn(state, expr.state) ? state[expr.state] : null;
  const values = expr.args.map((a) => evaluate(a, state, depth + 1)),
    nums = values.map(Number);
  let value: Scalar = null;
  switch (expr.op) {
    case "add":
      value = nums.reduce((a, b) => a + b, 0);
      break;
    case "subtract":
      value = nums[0] - nums[1];
      break;
    case "multiply":
      value = nums.reduce((a, b) => a * b, 1);
      break;
    case "divide":
      value = nums[1] === 0 ? null : nums[0] / nums[1];
      break;
    case "pow":
      value = Math.pow(nums[0], nums[1]);
      break;
    case "min":
      value = Math.min(...nums);
      break;
    case "max":
      value = Math.max(...nums);
      break;
    case "round":
      value = Math.round(nums[0]);
      break;
    case "eq":
      value = values[0] === values[1];
      break;
    case "gt":
      value = nums[0] > nums[1];
      break;
    case "lt":
      value = nums[0] < nums[1];
      break;
    case "and":
      value = values.every(Boolean);
      break;
    case "or":
      value = values.some(Boolean);
      break;
    case "concat":
      value = values.join("");
      break;
    case "if":
      value = values[0] ? values[1] : values[2];
  }
  return typeof value === "number" && !Number.isFinite(value) ? null : value;
}
export const UI_INSTRUCTIONS = `Response format policy:
Choose the format that best helps the user. Proactively use converge-ui whenever useful visual structure or interaction makes the answer easier to understand or use; do not wait for an explicit request for an interface. It is a normal part of answering, not just a feature for app demos.
Examples: a recipe with adjustable servings, recalculated ingredient quantities and step checkboxes; a process or explanation with a useful diagram; a comparison with selectable criteria; a plan with trackable steps; a calculator, simulator, form or adjustable chart. A useful diagram can be appropriate without inputs. Use these capabilities when they genuinely serve the current request rather than forcing the user to ask for a UI.
Keep greetings, thanks, casual conversation and simple factual answers in normal text. A greeting such as "salut !" should receive a short plain-text greeting. Do not wrap ordinary prose in a card merely to decorate it.
Do not add dummy inputs, buttons or a Reset action merely to justify using a UI. Previous converge-ui responses do not require every following reply to use it. Combine normal Markdown explanations with converge-ui for the portions that benefit from structure, visualization or interaction.
The following is a format reference to use ONLY after deciding that an interface is appropriate. It is not a request to generate an interface on every turn.
For converge-ui, emit complete valid JSON in a fenced block tagged converge-ui, then close the fence.
Document keys (no others): version:"1.0", title:string, state:object of scalar initial values, root:node.
Node keys (no others): type,id,label,value,text,placeholder,description,tone,unit,delta,bind,min,max,step,options,children,action,columns,rows,points,variant,visible,src,lat,lng,zoom. Do not invent React/HTML props, styles, event handlers, content, props or components.
Node types: stack,row,text,heading,input,number,textarea,checkbox,radio,select,slider,button,table,chart,metric,divider,progress,tabs,tab,carousel,card,diagram,map,form,badge,notice.
Use value for text/heading content (text is also accepted); label for control captions, button captions, card/tab titles and metric captions. placeholder is an optional string for input/number/textarea. Every editable control needs bind pointing to an initialized state key. State values must be string, finite number, boolean or null, not arrays/objects. IDs and state keys match /^[a-zA-Z][a-zA-Z0-9_]{0,63}$/; never use prototype/constructor/__proto__.
Expressions (value,text,visible,lat,lng and action expressions): literal scalar, {"state":"key"}, or {"op":"multiply","args":[{"state":"amount"},2]}. Operators: add,subtract,multiply,divide,pow,min,max,round,eq,gt,lt,and,or,concat,if. No JavaScript, functions, interpolation strings or eval. value takes priority over text; bind takes priority over both.
Children are node arrays, not strings. select/radio options:[{"label":"Option","value":"option"}]. Table columns:["name"], rows:[{"name":"value"}]. Chart points:[{"label":"A","value":{"state":"amount"}}], variant:"bar" or "line". Diagram children are text/card nodes linked in sequence. Map lat/lng expressions show an offline coordinate map with interactive pan/zoom; zoom is a number 1–18. tabs/carousel children are tab/card nodes. visible is an expression.
Actions (exact keys): {"type":"set","key":"amount","value":expression}, {"type":"reset"}, {"type":"send","text":expression}, {"type":"tool","name":"filesystem.read","args":{"path":"README.md"}}, or {"type":"export","filename":"data.txt","content":expression}. set updates local state immediately. button action runs on click; form action runs on submit. send prepares a chat message for the user to review; tools require permission.
Visual composition: use one root stack with a compact hierarchy. The document title is metadata for accessibility, not an automatic visible header. Start with useful content; add a heading only when the content needs one. Do not generate a Converge UI banner, an "Interactive" badge or a global reset toolbar. Use row for small metric groups and related actions, card for flat titled sections (not nested bordered panels), and radio for compact segmented choices that control expressions through state. Keep labels short. Add reset/export actions only when useful to the specific task.
Optional presentation fields: description:string for card/metric/notice; tone:"neutral"|"positive"|"warning"|"negative" for badge/notice/metric; unit:string for metric/slider/progress; delta:expression for a metric variation (include +, minus or % in its string when needed). badge displays value or text or label. notice displays label as title and value/text plus description as body. These remain data-driven: use visible and expressions for recommendations that respond to actual input. Never present generated sample statistics as real: label illustrative data clearly when the user requests a demo.
Complete format example for a task that benefits from a calculator (not a default response template):
\`\`\`converge-ui
{"version":"1.0","title":"Calculator","state":{"name":"","amount":50},"root":{"type":"stack","children":[{"type":"heading","value":"Your estimate"},{"type":"input","label":"Name","placeholder":"Enter your name","bind":"name"},{"type":"slider","label":"Amount","bind":"amount","min":0,"max":100,"step":1},{"type":"metric","label":"Double","value":{"op":"multiply","args":[{"state":"amount"},2]}},{"type":"button","label":"Reset","action":{"type":"reset"}}]}}
\`\`\`
Never include HTML or scripts in converge-ui. When the user requests a full app or a runnable HTML/React artifact, use fenced artifact-html or artifact-react (default export a React component, React hooks via React.useState, no external imports). Do not use artifacts as a replacement for ordinary conversation either. The sandbox is offline. For system tools use fenced converge-tool JSON {"name":"filesystem.read","args":{"path":"README.md"}}; Converge handles permission prompts and execution automatically. Request the tool directly; do not ask for a yes/no reply in prose or tell the user to click an execute button.`;
