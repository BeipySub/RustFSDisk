import { Linter } from "eslint";

//#region src/index.d.ts
type FlatConfig = Linter.Config;
declare function defineConfig(config?: FlatConfig[]): Promise<FlatConfig[]>;
//#endregion
export { defineConfig };