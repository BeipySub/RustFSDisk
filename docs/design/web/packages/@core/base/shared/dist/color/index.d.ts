import { TinyColor } from "@ctrl/tinycolor";

//#region src/color/color.d.ts
declare function isDarkColor(color: string): boolean;
declare function isLightColor(color: string): boolean;
//#endregion
//#region src/color/convert.d.ts
declare function convertToHsl(color: string): string;
declare function convertToHslCssVar(color: string): string;
declare function convertToRgb(str: string): string;
declare function isValidColor(color?: string): boolean;
//#endregion
//#region src/color/generator.d.ts
interface ColorItem {
  alias?: string;
  color: string;
  name: string;
}
declare function generatorColorVariables(colorItems: ColorItem[]): Record<string, string>;
//#endregion
export { TinyColor, convertToHsl, convertToHslCssVar, convertToRgb, generatorColorVariables, isDarkColor, isLightColor, isValidColor };