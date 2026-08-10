import js from "@eslint/js";
import pluginUnusedImports from "eslint-plugin-unused-imports";
import globals from "globals";
//#region src/configs/ignores.ts
async function ignores() {
	return [{ ignores: [
		"**/node_modules",
		"**/dist",
		"**/dist-*",
		"**/*-dist",
		"**/.husky",
		"**/.nitro",
		"**/.output",
		"**/Dockerfile",
		"**/package-lock.json",
		"**/yarn.lock",
		"**/pnpm-lock.yaml",
		"**/bun.lockb",
		"**/output",
		"**/coverage",
		"**/temp",
		"**/.temp",
		"**/tmp",
		"**/.tmp",
		"**/.history",
		"**/.turbo",
		"**/.nuxt",
		"**/.next",
		"**/.vercel",
		"**/.changeset",
		"**/.idea",
		"**/.cache",
		"**/.output",
		"**/.vite-inspect",
		"**/CHANGELOG*.md",
		"**/*.min.*",
		"**/LICENSE*",
		"**/__snapshots__",
		"**/*.snap",
		"**/fixtures/**",
		"**/.vitepress/cache/**",
		"**/auto-import?(s).d.ts",
		"**/components.d.ts",
		"**/vite.config.mts.*",
		"**/*.sh",
		"**/*.ttf",
		"**/*.woff",
		"**/.github",
		"**/lefthook.yml",
		"**/.agent/**",
		"**/.agents/**",
		"**/.codex/**",
		"**/.claude/**",
		"**/.cursor/**"
	] }];
}
//#endregion
//#region src/configs/javascript.ts
const rulesCoveredByOxlint$2 = new Set([
	"constructor-super",
	"for-direction",
	"getter-return",
	"no-async-promise-executor",
	"no-case-declarations",
	"no-class-assign",
	"no-compare-neg-zero",
	"no-cond-assign",
	"no-const-assign",
	"no-constant-binary-expression",
	"no-constant-condition",
	"no-debugger",
	"no-delete-var",
	"no-dupe-args",
	"no-dupe-class-members",
	"no-dupe-else-if",
	"no-dupe-keys",
	"no-duplicate-case",
	"no-empty",
	"no-empty-character-class",
	"no-empty-pattern",
	"no-empty-static-block",
	"no-ex-assign",
	"no-extra-boolean-cast",
	"no-fallthrough",
	"no-func-assign",
	"no-global-assign",
	"no-import-assign",
	"no-invalid-regexp",
	"no-irregular-whitespace",
	"no-loss-of-precision",
	"no-misleading-character-class",
	"no-new-native-nonconstructor",
	"no-nonoctal-decimal-escape",
	"no-obj-calls",
	"no-prototype-builtins",
	"no-redeclare",
	"no-regex-spaces",
	"no-self-assign",
	"no-setter-return",
	"no-shadow-restricted-names",
	"no-sparse-arrays",
	"no-this-before-super",
	"no-unreachable",
	"no-unsafe-finally",
	"no-unsafe-negation",
	"no-unsafe-optional-chaining",
	"no-unused-labels",
	"no-unused-private-class-members",
	"no-unused-vars",
	"no-useless-backreference",
	"no-useless-catch",
	"no-useless-escape",
	"no-with",
	"require-yield",
	"use-isnan",
	"valid-typeof"
]);
async function javascript() {
	const recommendedRules = Object.fromEntries(Object.entries(js.configs.recommended.rules).filter(([ruleName]) => !rulesCoveredByOxlint$2.has(ruleName)));
	return [{
		languageOptions: {
			ecmaVersion: "latest",
			globals: {
				...globals.browser,
				...globals.es2021,
				...globals.node,
				document: "readonly",
				navigator: "readonly",
				window: "readonly"
			},
			parserOptions: {
				ecmaFeatures: { jsx: true },
				ecmaVersion: "latest",
				sourceType: "module"
			},
			sourceType: "module"
		},
		linterOptions: { reportUnusedDisableDirectives: true },
		plugins: { "unused-imports": pluginUnusedImports },
		rules: {
			...recommendedRules,
			"dot-notation": ["error", { allowKeywords: true }],
			"keyword-spacing": "off",
			"no-control-regex": "error",
			"no-empty-function": "off",
			"no-octal": "error",
			"no-octal-escape": "error",
			"no-restricted-properties": [
				"error",
				{
					message: "Use `Object.getPrototypeOf` or `Object.setPrototypeOf` instead.",
					property: "__proto__"
				},
				{
					message: "Use `Object.defineProperty` instead.",
					property: "__defineGetter__"
				},
				{
					message: "Use `Object.defineProperty` instead.",
					property: "__defineSetter__"
				},
				{
					message: "Use `Object.getOwnPropertyDescriptor` instead.",
					property: "__lookupGetter__"
				},
				{
					message: "Use `Object.getOwnPropertyDescriptor` instead.",
					property: "__lookupSetter__"
				}
			],
			"no-restricted-syntax": [
				"error",
				"DebuggerStatement",
				"LabeledStatement",
				"WithStatement",
				"TSEnumDeclaration[const=true]",
				"TSExportAssignment"
			],
			"no-undef-init": "error",
			"no-undef": "off",
			"no-unreachable-loop": "error",
			"object-shorthand": [
				"error",
				"always",
				{
					avoidQuotes: true,
					ignoreConstructors: false
				}
			],
			"one-var": ["error", { initialized: "never" }],
			"prefer-arrow-callback": ["error", {
				allowNamedFunctions: false,
				allowUnboundThis: true
			}],
			"prefer-regex-literals": ["error", { disallowRedundantWrapping: true }],
			"spaced-comment": "error",
			"space-before-function-paren": "off",
			"unused-imports/no-unused-imports": "error",
			"unused-imports/no-unused-vars": ["error", {
				args: "after-used",
				argsIgnorePattern: "^_",
				vars: "all",
				varsIgnorePattern: "^_"
			}]
		}
	}];
}
//#endregion
//#region src/util.ts
async function interopDefault(m) {
	const resolved = await m;
	return resolved.default || resolved;
}
//#endregion
//#region src/configs/jsonc.ts
async function jsonc() {
	return [
		{
			files: [
				"**/*.json",
				"**/*.json5",
				"**/*.jsonc",
				"*.code-workspace"
			],
			language: "jsonc/x",
			plugins: { jsonc: await interopDefault(import("eslint-plugin-jsonc")) },
			rules: {
				"jsonc/no-bigint-literals": "error",
				"jsonc/no-binary-expression": "error",
				"jsonc/no-binary-numeric-literals": "error",
				"jsonc/no-dupe-keys": "error",
				"jsonc/no-escape-sequence-in-identifier": "error",
				"jsonc/no-floating-decimal": "error",
				"jsonc/no-hexadecimal-numeric-literals": "error",
				"jsonc/no-infinity": "error",
				"jsonc/no-multi-str": "error",
				"jsonc/no-nan": "error",
				"jsonc/no-number-props": "error",
				"jsonc/no-numeric-separators": "error",
				"jsonc/no-octal": "error",
				"jsonc/no-octal-escape": "error",
				"jsonc/no-octal-numeric-literals": "error",
				"jsonc/no-parenthesized": "error",
				"jsonc/no-plus-sign": "error",
				"jsonc/no-regexp-literals": "error",
				"jsonc/no-sparse-arrays": "error",
				"jsonc/no-template-literals": "error",
				"jsonc/no-undefined-value": "error",
				"jsonc/no-unicode-codepoint-escapes": "error",
				"jsonc/no-useless-escape": "error",
				"jsonc/space-unary-ops": "error",
				"jsonc/valid-json-number": "error",
				"jsonc/vue-custom-block/no-parsing-error": "error"
			}
		},
		sortTsconfig(),
		sortPackageJson(),
		sortCspellJson()
	];
}
function sortPackageJson() {
	return {
		files: ["**/package.json"],
		rules: {
			"jsonc/sort-array-values": ["error", {
				order: { type: "asc" },
				pathPattern: "^files$|^pnpm.neverBuiltDependencies$"
			}],
			"jsonc/sort-keys": [
				"error",
				{
					order: [
						"name",
						"version",
						"description",
						"private",
						"keywords",
						"homepage",
						"bugs",
						"repository",
						"license",
						"author",
						"contributors",
						"categories",
						"funding",
						"type",
						"scripts",
						"files",
						"sideEffects",
						"bin",
						"main",
						"module",
						"unpkg",
						"jsdelivr",
						"types",
						"typesVersions",
						"imports",
						"exports",
						"publishConfig",
						"icon",
						"activationEvents",
						"contributes",
						"peerDependencies",
						"peerDependenciesMeta",
						"dependencies",
						"optionalDependencies",
						"devDependencies",
						"engines",
						"packageManager",
						"pnpm",
						"overrides",
						"resolutions",
						"husky",
						"simple-git-hooks",
						"lint-staged",
						"eslintConfig"
					],
					pathPattern: "^$"
				},
				{
					order: { type: "asc" },
					pathPattern: "^(?:dev|peer|optional|bundled)?[Dd]ependencies(Meta)?$"
				},
				{
					order: { type: "asc" },
					pathPattern: "^(?:resolutions|overrides|pnpm.overrides)$"
				},
				{
					order: [
						"types",
						"import",
						"require",
						"default"
					],
					pathPattern: "^exports.*$"
				}
			]
		}
	};
}
function sortCspellJson() {
	return {
		files: ["**/cspell.json", "**/.cspell.json"],
		rules: { "jsonc/sort-array-values": ["error", {
			order: { type: "asc" },
			pathPattern: "^words$|^ignorePaths$"
		}] }
	};
}
function sortTsconfig() {
	return {
		files: [
			"**/tsconfig.json",
			"**/tsconfig.*.json",
			"internal/tsconfig/*.json"
		],
		rules: { "jsonc/sort-keys": [
			"error",
			{
				order: [
					"extends",
					"compilerOptions",
					"references",
					"files",
					"include",
					"exclude"
				],
				pathPattern: "^$"
			},
			{
				order: [
					"incremental",
					"composite",
					"tsBuildInfoFile",
					"disableSourceOfProjectReferenceRedirect",
					"disableSolutionSearching",
					"disableReferencedProjectLoad",
					"target",
					"jsx",
					"jsxFactory",
					"jsxFragmentFactory",
					"jsxImportSource",
					"lib",
					"moduleDetection",
					"noLib",
					"reactNamespace",
					"useDefineForClassFields",
					"emitDecoratorMetadata",
					"experimentalDecorators",
					"baseUrl",
					"rootDir",
					"rootDirs",
					"customConditions",
					"module",
					"moduleResolution",
					"moduleSuffixes",
					"noResolve",
					"paths",
					"resolveJsonModule",
					"resolvePackageJsonExports",
					"resolvePackageJsonImports",
					"typeRoots",
					"types",
					"allowArbitraryExtensions",
					"allowImportingTsExtensions",
					"allowUmdGlobalAccess",
					"allowJs",
					"checkJs",
					"maxNodeModuleJsDepth",
					"strict",
					"strictBindCallApply",
					"strictFunctionTypes",
					"strictNullChecks",
					"strictPropertyInitialization",
					"allowUnreachableCode",
					"allowUnusedLabels",
					"alwaysStrict",
					"exactOptionalPropertyTypes",
					"noFallthroughCasesInSwitch",
					"noImplicitAny",
					"noImplicitOverride",
					"noImplicitReturns",
					"noImplicitThis",
					"noPropertyAccessFromIndexSignature",
					"noUncheckedIndexedAccess",
					"noUnusedLocals",
					"noUnusedParameters",
					"useUnknownInCatchVariables",
					"declaration",
					"declarationDir",
					"declarationMap",
					"downlevelIteration",
					"emitBOM",
					"emitDeclarationOnly",
					"importHelpers",
					"importsNotUsedAsValues",
					"inlineSourceMap",
					"inlineSources",
					"mapRoot",
					"newLine",
					"noEmit",
					"noEmitHelpers",
					"noEmitOnError",
					"outDir",
					"outFile",
					"preserveConstEnums",
					"preserveValueImports",
					"removeComments",
					"sourceMap",
					"sourceRoot",
					"stripInternal",
					"allowSyntheticDefaultImports",
					"esModuleInterop",
					"forceConsistentCasingInFileNames",
					"isolatedModules",
					"preserveSymlinks",
					"verbatimModuleSyntax",
					"skipDefaultLibCheck",
					"skipLibCheck"
				],
				pathPattern: "^compilerOptions$"
			}
		] }
	};
}
//#endregion
//#region src/configs/node.ts
async function node() {
	return [
		{
			plugins: { n: await interopDefault(import("eslint-plugin-n")) },
			rules: {
				"n/handle-callback-err": ["error", "^(err|error)$"],
				"n/no-deprecated-api": "error",
				"n/no-extraneous-import": ["error", { allowModules: [
					"tsdown",
					"unplugin-vue",
					"@vben/vite-config",
					"vitest",
					"vite",
					"@vue/test-utils",
					"@playwright/test"
				] }],
				"n/no-unsupported-features/es-syntax": ["error", {
					ignores: [],
					version: ">=20.12.0"
				}],
				"n/prefer-global/buffer": ["error", "never"],
				"n/prefer-global/process": ["error", "never"],
				"n/process-exit-as-throw": "error"
			}
		},
		{
			files: [
				"**/__tests__/**/*.?([cm])[jt]s?(x)",
				"**/*.spec.?([cm])[jt]s?(x)",
				"**/*.test.?([cm])[jt]s?(x)",
				"**/*.bench.?([cm])[jt]s?(x)",
				"**/*.benchmark.?([cm])[jt]s?(x)"
			],
			rules: { "n/prefer-global/process": "off" }
		},
		{
			files: ["apps/backend-mock/**/**", "docs/**/**"],
			rules: {
				"n/no-extraneous-import": "off",
				"n/prefer-global/buffer": "off",
				"n/prefer-global/process": "off"
			}
		},
		{
			files: ["**/**/playwright.config.ts"],
			rules: {
				"n/prefer-global/buffer": "off",
				"n/prefer-global/process": "off"
			}
		},
		{
			files: ["scripts/**/*.?([cm])[jt]s?(x)", "internal/**/*.?([cm])[jt]s?(x)"],
			rules: { "n/prefer-global/process": "off" }
		}
	];
}
//#endregion
//#region src/configs/perfectionist.ts
async function perfectionist() {
	return [(await interopDefault(import("eslint-plugin-perfectionist"))).configs["recommended-natural"], { rules: {
		"perfectionist/sort-exports": ["error", {
			order: "asc",
			type: "natural"
		}],
		"perfectionist/sort-imports": ["error", {
			customGroups: [
				{
					selector: "type",
					groupName: "vben-core-type",
					elementNamePattern: "^@vben-core/.+"
				},
				{
					selector: "type",
					groupName: "vben-type",
					elementNamePattern: "^@vben/.+"
				},
				{
					selector: "type",
					groupName: "vue-type",
					elementNamePattern: [
						"^vue$",
						"^vue-.+",
						"^@vue/.+"
					]
				},
				{
					groupName: "vben",
					elementNamePattern: "^@vben/.+"
				},
				{
					groupName: "vben-core",
					elementNamePattern: "^@vben-core/.+"
				},
				{
					groupName: "vue",
					elementNamePattern: [
						"^vue$",
						"^vue-.+",
						"^@vue/.+"
					]
				}
			],
			environment: "node",
			groups: [
				[
					"type-external",
					"type-builtin",
					"type-import"
				],
				"vue-type",
				"vben-type",
				"vben-core-type",
				[
					"type-parent",
					"type-sibling",
					"type-index"
				],
				["type-internal"],
				"value-builtin",
				"vue",
				"vben",
				"vben-core",
				"value-external",
				"value-internal",
				[
					"value-parent",
					"value-sibling",
					"value-index"
				],
				"side-effect",
				"side-effect-style",
				"style",
				"ts-equals-import",
				"unknown"
			],
			internalPattern: ["^#/.+"],
			newlinesBetween: 1,
			order: "asc",
			type: "natural"
		}],
		"perfectionist/sort-modules": "off",
		"perfectionist/sort-named-exports": ["error", {
			order: "asc",
			type: "natural"
		}],
		"perfectionist/sort-objects": ["off", {
			customGroups: {
				items: "items",
				list: "list",
				children: "children"
			},
			groups: [
				"unknown",
				"items",
				"list",
				"children"
			],
			ignorePattern: ["children"],
			order: "asc",
			type: "natural"
		}]
	} }];
}
//#endregion
//#region src/configs/pnpm.ts
async function pnpm() {
	const [pluginPnpm, parserPnpm] = await Promise.all([interopDefault(import("eslint-plugin-pnpm")), interopDefault(import("yaml-eslint-parser"))]);
	return [{
		files: ["package.json", "**/package.json"],
		language: "jsonc/x",
		plugins: { pnpm: pluginPnpm },
		rules: {
			"pnpm/json-enforce-catalog": "error",
			"pnpm/json-prefer-workspace-settings": "error",
			"pnpm/json-valid-catalog": "error"
		}
	}, {
		files: ["pnpm-workspace.yaml"],
		languageOptions: { parser: parserPnpm },
		plugins: { pnpm: pluginPnpm },
		rules: {
			"pnpm/yaml-no-duplicate-catalog-item": "error",
			"pnpm/yaml-no-unused-catalog-item": "error"
		}
	}];
}
//#endregion
//#region src/configs/typescript.ts
const rulesCoveredByOxlint$1 = new Set([
	"@typescript-eslint/ban-ts-comment",
	"@typescript-eslint/no-non-null-assertion",
	"@typescript-eslint/no-unused-expressions",
	"@typescript-eslint/no-unused-vars",
	"@typescript-eslint/triple-slash-reference"
]);
async function typescript() {
	const [pluginTs, parserTs] = await Promise.all([interopDefault(import("@typescript-eslint/eslint-plugin")), interopDefault(import("@typescript-eslint/parser"))]);
	const strictRules = Object.fromEntries(Object.entries(pluginTs.configs.strict?.rules ?? {}).filter(([ruleName]) => !rulesCoveredByOxlint$1.has(ruleName)));
	return [{
		files: ["**/*.?([cm])[jt]s?(x)"],
		languageOptions: {
			parser: parserTs,
			parserOptions: {
				createDefaultProgram: false,
				ecmaFeatures: { jsx: true },
				ecmaVersion: "latest",
				extraFileExtensions: [".vue"],
				jsxPragma: "React",
				project: "./tsconfig.*.json",
				sourceType: "module"
			}
		},
		plugins: { "@typescript-eslint": pluginTs },
		rules: {
			...pluginTs.configs["eslint-recommended"]?.overrides?.[0]?.rules,
			...strictRules,
			"@typescript-eslint/consistent-type-definitions": "off",
			"@typescript-eslint/explicit-function-return-type": "off",
			"@typescript-eslint/explicit-module-boundary-types": "off",
			"@typescript-eslint/no-explicit-any": "off",
			"@typescript-eslint/no-namespace": "off",
			"@typescript-eslint/no-use-before-define": "off",
			"unused-imports/no-unused-vars": "off"
		}
	}];
}
//#endregion
//#region src/configs/unicorn.ts
const rulesCoveredByOxlint = new Set([
	"unicorn/consistent-function-scoping",
	"unicorn/no-process-exit",
	"unicorn/prefer-global-this",
	"unicorn/prefer-module"
]);
async function unicorn() {
	const pluginUnicorn = await interopDefault(import("eslint-plugin-unicorn"));
	const recommendedRules = Object.fromEntries(Object.entries(pluginUnicorn.configs.recommended.rules ?? {}).filter(([ruleName]) => !rulesCoveredByOxlint.has(ruleName)));
	return [{
		plugins: { unicorn: pluginUnicorn },
		rules: {
			...recommendedRules,
			"unicorn/better-regex": "off",
			"unicorn/consistent-destructuring": "off",
			"unicorn/expiring-todo-comments": "off",
			"unicorn/filename-case": "off",
			"unicorn/import-style": "off",
			"unicorn/no-array-for-each": "off",
			"unicorn/no-null": "off",
			"unicorn/no-useless-undefined": "off",
			"unicorn/prefer-at": "off",
			"unicorn/prefer-dom-node-text-content": "off",
			"unicorn/prefer-export-from": ["error", { ignoreUsedVariables: true }],
			"unicorn/prefer-top-level-await": "off",
			"unicorn/prevent-abbreviations": "off"
		}
	}];
}
//#endregion
//#region src/configs/vue.ts
async function vue() {
	const [pluginVue, parserVue, parserTs] = await Promise.all([
		interopDefault(import("eslint-plugin-vue")),
		interopDefault(import("vue-eslint-parser")),
		interopDefault(import("@typescript-eslint/parser"))
	]);
	const flatEssential = pluginVue.configs?.["flat/essential"] || [];
	const flatStronglyRecommended = pluginVue.configs?.["flat/strongly-recommended"] || [];
	const flatRecommended = pluginVue.configs?.["flat/recommended"] || [];
	return [
		...flatEssential,
		...flatStronglyRecommended,
		...flatRecommended,
		{
			files: ["**/*.vue"],
			languageOptions: {
				parser: parserVue,
				parserOptions: {
					ecmaFeatures: { jsx: true },
					extraFileExtensions: [".vue"],
					parser: parserTs,
					sourceType: "module"
				}
			},
			plugins: { vue: pluginVue },
			processor: pluginVue.processors?.[".vue"],
			rules: {
				...pluginVue.configs?.base?.rules,
				"vue/attribute-hyphenation": [
					"error",
					"always",
					{ ignore: [] }
				],
				"vue/attributes-order": "off",
				"vue/block-order": ["error", { order: [
					"script",
					"template",
					"style"
				] }],
				"vue/component-name-in-template-casing": ["error", "PascalCase"],
				"vue/component-options-name-casing": ["error", "PascalCase"],
				"vue/custom-event-name-casing": ["error", "camelCase"],
				"vue/define-macros-order": ["error", { order: [
					"defineOptions",
					"defineProps",
					"defineEmits",
					"defineSlots"
				] }],
				"vue/dot-location": ["error", "property"],
				"vue/dot-notation": ["error", { allowKeywords: true }],
				"vue/eqeqeq": ["error", "smart"],
				"vue/html-closing-bracket-newline": "error",
				"vue/html-indent": "off",
				"vue/html-quotes": ["error", "double"],
				"vue/html-self-closing": ["error", {
					html: {
						component: "always",
						normal: "never",
						void: "always"
					},
					math: "always",
					svg: "always"
				}],
				"vue/max-attributes-per-line": "off",
				"vue/multi-word-component-names": "off",
				"vue/multiline-html-element-content-newline": "error",
				"vue/no-empty-pattern": "error",
				"vue/no-extra-parens": ["error", "functions"],
				"vue/no-irregular-whitespace": "error",
				"vue/no-loss-of-precision": "error",
				"vue/no-reserved-component-names": "off",
				"vue/no-restricted-syntax": [
					"error",
					"DebuggerStatement",
					"LabeledStatement",
					"WithStatement"
				],
				"vue/no-restricted-v-bind": ["error", "/^v-/"],
				"vue/no-sparse-arrays": "error",
				"vue/no-unused-refs": "error",
				"vue/no-useless-v-bind": "error",
				"vue/object-shorthand": [
					"error",
					"always",
					{
						avoidQuotes: true,
						ignoreConstructors: false
					}
				],
				"vue/one-component-per-file": "error",
				"vue/prefer-separate-static-class": "error",
				"vue/prefer-template": "error",
				"vue/prop-name-casing": ["error", "camelCase"],
				"vue/require-default-prop": "error",
				"vue/require-explicit-emits": "error",
				"vue/require-prop-types": "off",
				"vue/singleline-html-element-content-newline": "off",
				"vue/space-infix-ops": "error",
				"vue/space-unary-ops": ["error", {
					nonwords: false,
					words: true
				}],
				"vue/v-on-event-hyphenation": [
					"error",
					"always",
					{
						autofix: true,
						ignore: []
					}
				]
			}
		}
	];
}
//#endregion
//#region src/configs/yaml.ts
async function yaml() {
	const [pluginYaml, parserYaml] = await Promise.all([interopDefault(import("eslint-plugin-yml")), interopDefault(import("yaml-eslint-parser"))]);
	return [{
		files: ["**/*.y?(a)ml"],
		plugins: { yaml: pluginYaml },
		languageOptions: { parser: parserYaml },
		rules: {
			"style/spaced-comment": "off",
			"yaml/block-mapping": "error",
			"yaml/block-sequence": "error",
			"yaml/no-empty-key": "error",
			"yaml/no-empty-sequence-entry": "error",
			"yaml/no-irregular-whitespace": "error",
			"yaml/plain-scalar": "error",
			"yaml/vue-custom-block/no-parsing-error": "error",
			"yaml/block-mapping-question-indicator-newline": "error",
			"yaml/block-sequence-hyphen-indicator-newline": "error",
			"yaml/flow-mapping-curly-newline": "error",
			"yaml/flow-mapping-curly-spacing": "error",
			"yaml/flow-sequence-bracket-newline": "error",
			"yaml/flow-sequence-bracket-spacing": "error",
			"yaml/indent": ["error", 2],
			"yaml/key-spacing": "error",
			"yaml/no-tab-indent": "error",
			"yaml/quotes": ["error", {
				avoidEscape: true,
				prefer: "single"
			}],
			"yaml/spaced-comment": "error"
		}
	}, {
		files: ["pnpm-workspace.yaml"],
		rules: { "yaml/sort-keys": [
			"error",
			{
				order: [
					"packages",
					"overrides",
					"patchedDependencies",
					"hoistPattern",
					"catalog",
					"catalogs",
					"allowedDeprecatedVersions",
					"allowNonAppliedPatches",
					"configDependencies",
					"ignoredBuiltDependencies",
					"ignoredOptionalDependencies",
					"neverBuiltDependencies",
					"onlyBuiltDependencies",
					"onlyBuiltDependenciesFile",
					"packageExtensions",
					"peerDependencyRules",
					"supportedArchitectures"
				],
				pathPattern: "^$"
			},
			{
				order: { type: "asc" },
				pathPattern: ".*"
			}
		] }
	}];
}
//#endregion
//#region src/custom-config.ts
const restrictedImportIgnores = ["**/vite.config.mts"];
const customConfig = [
	{
		files: ["packages/@core/ui-kit/shadcn-ui/**/**"],
		rules: { "vue/require-default-prop": "off" }
	},
	{
		files: [
			"apps/**/**",
			"packages/effects/**/**",
			"packages/utils/**/**",
			"packages/types/**/**",
			"packages/locales/**/**"
		],
		ignores: restrictedImportIgnores,
		rules: { "perfectionist/sort-interfaces": "off" }
	},
	{
		files: ["apps/**/**"],
		ignores: restrictedImportIgnores,
		rules: { "no-restricted-imports": ["error", { patterns: [
			{
				group: ["#/api/*"],
				message: "The #/api package cannot be imported, please use the @core package itself"
			},
			{
				group: ["#/layouts/*"],
				message: "The #/layouts package cannot be imported, please use the @core package itself"
			},
			{
				group: ["#/locales/*"],
				message: "The #/locales package cannot be imported, please use the @core package itself"
			},
			{
				group: ["#/stores/*"],
				message: "The #/stores package cannot be imported, please use the @core package itself"
			}
		] }] }
	},
	{
		files: ["packages/@core/**/**"],
		ignores: restrictedImportIgnores,
		rules: { "no-restricted-imports": ["error", { patterns: [{
			group: ["@vben/*"],
			message: "The @core package cannot import the @vben package, please use the @core package itself"
		}] }] }
	},
	{
		files: ["packages/@core/base/**/**"],
		ignores: restrictedImportIgnores,
		rules: { "no-restricted-imports": ["error", { patterns: [{
			group: ["@vben/*", "@vben-core/*"],
			message: "The @vben-core/shared package cannot import the @vben package, please use the @core/shared package itself"
		}] }] }
	},
	{
		files: [
			"packages/types/**/**",
			"packages/utils/**/**",
			"packages/icons/**/**",
			"packages/constants/**/**",
			"packages/styles/**/**",
			"packages/stores/**/**",
			"packages/preferences/**/**",
			"packages/locales/**/**"
		],
		ignores: restrictedImportIgnores,
		rules: { "no-restricted-imports": ["error", { patterns: [{
			group: ["@vben/*"],
			message: "The @vben package cannot be imported, please use the @core package itself"
		}] }] }
	},
	{
		files: ["apps/backend-mock/**/**", "docs/**/**"],
		rules: { "no-console": "off" }
	},
	{
		files: ["**/**/playwright.config.ts"],
		rules: { "no-console": "off" }
	},
	{
		files: ["internal/**/**", "scripts/**/**"],
		rules: { "no-console": "off" }
	},
	{
		files: ["packages/@core/base/shared/src/utils/inference.ts"],
		rules: { "vue/prefer-import-from-vue": "off" }
	}
];
//#endregion
//#region src/index.ts
async function defineConfig(config = []) {
	const configs = [
		vue(),
		javascript(),
		ignores(),
		typescript(),
		jsonc(),
		node(),
		perfectionist(),
		unicorn(),
		yaml(),
		pnpm(),
		...customConfig,
		...config
	];
	return (await Promise.all(configs)).flat();
}
//#endregion
export { defineConfig };
