import { defineConfig as defineConfig$1 } from "oxfmt";

//#region src/index.d.ts
type OxfmtConfig = Parameters<typeof defineConfig$1>[0];
declare const oxfmtConfig: OxfmtConfig;
declare function defineConfig(config?: OxfmtConfig): OxfmtConfig;
//#endregion
export { type OxfmtConfig, defineConfig, oxfmtConfig };