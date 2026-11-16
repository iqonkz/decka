// Style sheets are imported for their side effect (the host's bundler handles them).
declare module "*.css";
// Weather icons come in as the text of the SVG file (DESIGN.md 2.3).
declare module "*.svg?raw" {
  const text: string;
  export default text;
}
