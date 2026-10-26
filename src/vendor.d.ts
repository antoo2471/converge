declare module "@babel/standalone" {
  export function transform(
    code: string,
    options: unknown,
  ): { code: string | null };
}
declare module "*.txt?raw" {
  const value: string;
  export default value;
}
