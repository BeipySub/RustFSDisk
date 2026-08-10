import { defineConfig as defineConfig$1 } from "oxfmt";
//#region src/index.ts
const oxfmtConfig = defineConfig$1({
	printWidth: 80,
	proseWrap: "never",
	semi: true,
	singleQuote: true,
	sortPackageJson: false,
	trailingComma: "all",
	overrides: [{
		files: [
			"*.json",
			"*.json5",
			"*.jsonc",
			"*.code-workspace",
			"**/*.json",
			"**/*.json5",
			"**/*.jsonc",
			"**/*.code-workspace"
		],
		options: { trailingComma: "none" }
	}]
});
function defineConfig(config = {}) {
	return defineConfig$1({
		...oxfmtConfig,
		...config
	});
}
//#endregion
export { defineConfig, oxfmtConfig };
