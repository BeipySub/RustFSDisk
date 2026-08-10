import { OxlintConfig, OxlintConfig as OxlintConfig$1 } from "oxlint";

//#region src/configs/command.d.ts
declare const command: OxlintConfig$1;
//#endregion
//#region src/configs/comments.d.ts
declare const comments: OxlintConfig$1;
//#endregion
//#region src/configs/ignores.d.ts
declare const ignores: OxlintConfig$1;
//#endregion
//#region src/configs/import.d.ts
declare const importPluginConfig: OxlintConfig$1;
//#endregion
//#region src/configs/javascript.d.ts
declare const javascript: OxlintConfig$1;
//#endregion
//#region src/configs/node.d.ts
declare const node: OxlintConfig$1;
//#endregion
//#region src/configs/overrides.d.ts
declare const overrides: OxlintConfig$1;
//#endregion
//#region src/configs/plugins.d.ts
declare const plugins: OxlintConfig$1;
//#endregion
//#region src/configs/tailwindcss.d.ts
declare const tailwindcss: OxlintConfig$1;
//#endregion
//#region src/configs/test.d.ts
declare const test: OxlintConfig$1;
//#endregion
//#region src/configs/typescript.d.ts
declare const typescript: OxlintConfig$1;
//#endregion
//#region src/configs/unicorn.d.ts
declare const unicorn: OxlintConfig$1;
//#endregion
//#region src/configs/vue.d.ts
declare const vue: OxlintConfig$1;
//#endregion
//#region src/configs/index.d.ts
declare function mergeOxlintConfigs(...configs: OxlintConfig$1[]): OxlintConfig$1;
declare const oxlintConfig: OxlintConfig$1;
//#endregion
//#region src/index.d.ts
type VbenOxlintConfig = Omit<OxlintConfig, 'extends'> & {
  extends?: OxlintConfig[];
};
declare function defineConfig(config?: VbenOxlintConfig): OxlintConfig;
//#endregion
export { type OxlintConfig, type VbenOxlintConfig, command, comments, defineConfig, ignores, importPluginConfig, javascript, mergeOxlintConfigs, node, overrides, oxlintConfig, plugins, tailwindcss, test, typescript, unicorn, vue };