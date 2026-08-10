import fs, { existsSync } from "node:fs";
import path, { join, relative } from "node:path";
import { colors, consola, dateUtil, findMonorepoRoot, fs as fs$1, generatorContentHash, getPackage, getPackages, readPackageJSON } from "@vben/node-utils";
import { NodePackageImporter } from "sass-embedded";
import { defineConfig as defineConfig$1, loadEnv, mergeConfig } from "vite";
import viteVueI18nPlugin from "@intlify/unplugin-vue-i18n/vite";
import tailwindcss from "@tailwindcss/vite";
import viteVue from "@vitejs/plugin-vue";
import viteVueJsx from "@vitejs/plugin-vue-jsx";
import { visualizer as viteVisualizerPlugin } from "rollup-plugin-visualizer";
import viteDtsPlugin from "unplugin-dts/vite";
import viteCompressPlugin from "vite-plugin-compression";
import { VitePWA } from "vite-plugin-pwa";
import viteVueDevTools from "vite-plugin-vue-devtools";
import fsp from "node:fs/promises";
import archiver from "archiver";
import dotenv from "dotenv";
import { minify } from "html-minifier-terser";
import { Generator } from "@jspm/generator";
import { load } from "cheerio";
import { fileURLToPath } from "node:url";
import { readWorkspaceManifest } from "@pnpm/workspace.read-manifest";
import { EOL } from "node:os";
import getPort from "get-port";
import { build, createDevServer, createNitro, prepare } from "nitropack";
import { VxeResolver, lazyImport } from "vite-plugin-lazy-import";
//#region src/options.ts
const isDevelopment = process.env.NODE_ENV === "development";
const getDefaultPwaOptions = (name) => ({ manifest: {
	description: "Vben Admin is a modern admin dashboard template based on Vue 3. ",
	icons: [{
		sizes: "192x192",
		src: "https://unpkg.com/@vbenjs/static-source@0.1.7/source/pwa-icon-192.png",
		type: "image/png"
	}, {
		sizes: "512x512",
		src: "https://unpkg.com/@vbenjs/static-source@0.1.7/source/pwa-icon-512.png",
		type: "image/png"
	}],
	name: `${name}${isDevelopment ? " dev" : ""}`,
	short_name: `${name}${isDevelopment ? " dev" : ""}`
} });
/**
* importmap CDN 暂时不开启，因为有些包不支持，且网络不稳定
*/
const defaultImportmapOptions = {
	defaultProvider: "esm.sh",
	importmap: [
		{ name: "vue" },
		{ name: "pinia" },
		{ name: "vue-router" },
		{ name: "dayjs" },
		{ name: "vue-demi" }
	]
};
//#endregion
//#region src/plugins/archiver.ts
const viteArchiverPlugin = (options = {}) => {
	return {
		apply: "build",
		closeBundle: {
			handler() {
				const { name = "dist", outputDir = "." } = options;
				setTimeout(async () => {
					const folderToZip = "dist";
					const zipOutputDir = join(process.cwd(), outputDir);
					const zipOutputPath = join(zipOutputDir, `${name}.zip`);
					try {
						await fsp.mkdir(zipOutputDir, { recursive: true });
					} catch {}
					try {
						await zipFolder(folderToZip, zipOutputPath);
						console.log(`Folder has been zipped to: ${zipOutputPath}`);
					} catch (error) {
						console.error("Error zipping folder:", error);
					}
				}, 0);
			},
			order: "post"
		},
		enforce: "post",
		name: "vite:archiver"
	};
};
async function zipFolder(folderPath, outputPath) {
	return new Promise((resolve, reject) => {
		const output = fs.createWriteStream(outputPath);
		const archive = archiver("zip", { zlib: { level: 9 } });
		output.on("close", () => {
			console.log(`ZIP file created: ${outputPath} (${archive.pointer()} total bytes)`);
			resolve();
		});
		archive.on("error", (err) => {
			reject(err);
		});
		archive.pipe(output);
		archive.directory(folderPath, false);
		archive.finalize();
	});
}
//#endregion
//#region src/plugins/dayjs.ts
function viteDayjsPlugin() {
	return {
		name: "vite-dayjs-plugin",
		enforce: "pre",
		async resolveId(source, importer, options) {
			if (source.startsWith("dayjs/esm")) return null;
			if (source === "dayjs") return await this.resolve("dayjs/esm", importer, {
				skipSelf: true,
				...options
			});
			const pluginWithJs = source.match(/^dayjs\/plugin\/([^/]+)\.js$/);
			if (pluginWithJs) {
				const target = `dayjs/esm/plugin/${pluginWithJs[1]}/index.js`;
				return await this.resolve(target, importer, {
					skipSelf: true,
					...options
				});
			}
			const pluginBare = source.match(/^dayjs\/plugin\/([^/]+)$/);
			if (pluginBare) {
				const target = `dayjs/esm/plugin/${pluginBare[1]}`;
				return await this.resolve(target, importer, {
					skipSelf: true,
					...options
				});
			}
			const localeWithJs = source.match(/^dayjs\/locale\/([^/]+)\.js$/);
			if (localeWithJs) {
				const target = `dayjs/esm/locale/${localeWithJs[1]}.js`;
				return await this.resolve(target, importer, {
					skipSelf: true,
					...options
				});
			}
			const localeBare = source.match(/^dayjs\/locale\/([^/]+)$/);
			if (localeBare) {
				const target = `dayjs/esm/locale/${localeBare[1]}`;
				return await this.resolve(target, importer, {
					skipSelf: true,
					...options
				});
			}
			return null;
		},
		config() {
			return { optimizeDeps: { exclude: ["dayjs"] } };
		}
	};
}
//#endregion
//#region src/utils/env.ts
const getBoolean = (value) => value === "true";
const getString = (value, fallback) => value ?? fallback;
const getNumber = (value, fallback) => Number(value) || fallback;
/**
* 获取当前环境下生效的配置文件名
*/
function getConfFiles() {
	const script = process.env.npm_lifecycle_script;
	const result = /--mode ([\d_a-z]+)/.exec(script);
	let mode = "production";
	if (result) mode = result[1];
	return [
		".env",
		".env.local",
		`.env.${mode}`,
		`.env.${mode}.local`
	];
}
/**
* Get the environment variables starting with the specified prefix
* @param match prefix
* @param confFiles ext
*/
async function loadEnv$1(match = "VITE_GLOB_", confFiles = getConfFiles()) {
	let envConfig = {};
	for (const confFile of confFiles) try {
		const confFilePath = join(process.cwd(), confFile);
		if (existsSync(confFilePath)) {
			const envPath = await fs$1.readFile(confFilePath, { encoding: "utf8" });
			const env = dotenv.parse(envPath);
			envConfig = {
				...envConfig,
				...env
			};
		}
	} catch (error) {
		console.error(`Error while parsing ${confFile}`, error);
	}
	const reg = new RegExp(`^(${match})`);
	Object.keys(envConfig).forEach((key) => {
		if (!reg.test(key)) Reflect.deleteProperty(envConfig, key);
	});
	return envConfig;
}
async function loadAndConvertEnv(match = "VITE_", confFiles = getConfFiles()) {
	const { VITE_APP_TITLE, VITE_ARCHIVER, VITE_BASE, VITE_COMPRESS, VITE_DEVTOOLS, VITE_INJECT_APP_LOADING, VITE_NITRO_MOCK, VITE_PORT, VITE_PWA, VITE_VISUALIZER } = await loadEnv$1(match, confFiles);
	const compressTypes = (VITE_COMPRESS ?? "").split(",").filter((item) => item === "brotli" || item === "gzip");
	return {
		appTitle: getString(VITE_APP_TITLE, "Vben Admin"),
		archiver: getBoolean(VITE_ARCHIVER),
		base: getString(VITE_BASE, "/"),
		compress: compressTypes.length > 0,
		compressTypes,
		devtools: getBoolean(VITE_DEVTOOLS),
		injectAppLoading: getBoolean(VITE_INJECT_APP_LOADING),
		nitroMock: getBoolean(VITE_NITRO_MOCK),
		port: getNumber(VITE_PORT, 5173),
		pwa: getBoolean(VITE_PWA),
		visualizer: getBoolean(VITE_VISUALIZER)
	};
}
//#endregion
//#region src/plugins/extra-app-config.ts
const GLOBAL_CONFIG_FILE_NAME = "_app.config.js";
const VBEN_ADMIN_PRO_APP_CONF = "_VBEN_ADMIN_PRO_APP_CONF_";
/**
* 用于将配置文件抽离出来并注入到项目中
* @returns
*/
async function viteExtraAppConfigPlugin({ isBuild, root }) {
	let publicPath;
	let source;
	if (!isBuild) return;
	const { version = "" } = await readPackageJSON(root);
	return {
		async configResolved(config) {
			publicPath = ensureTrailingSlash(config.base);
			source = await getConfigSource();
		},
		async generateBundle() {
			try {
				this.emitFile({
					fileName: GLOBAL_CONFIG_FILE_NAME,
					source,
					type: "asset"
				});
				console.log(colors.cyan(`✨configuration file is build successfully!`));
			} catch (error) {
				console.log(colors.red(`configuration file configuration file failed to package:\n${error}`));
			}
		},
		name: "vite:extra-app-config",
		async transformIndexHtml(html) {
			const hash = `v=${version}-${generatorContentHash(source, 8)}`;
			return {
				html,
				tags: [{
					attrs: { src: `${publicPath}${GLOBAL_CONFIG_FILE_NAME}?${hash}` },
					tag: "script"
				}]
			};
		}
	};
}
async function getConfigSource() {
	const config = await loadEnv$1();
	const windowVariable = `window.${VBEN_ADMIN_PRO_APP_CONF}`;
	let source = `${windowVariable}=${JSON.stringify(config)};`;
	source += `
    Object.freeze(${windowVariable});
    Object.defineProperty(window, "${VBEN_ADMIN_PRO_APP_CONF}", {
      configurable: false,
      writable: false,
    });
  `.replaceAll(/\s/g, "");
	return source;
}
function ensureTrailingSlash(path) {
	return path.endsWith("/") ? path : `${path}/`;
}
//#endregion
//#region src/plugins/html.ts
const HTML_MINIFY_OPTIONS = {
	collapseWhitespace: true,
	minifyCSS: true,
	minifyJS: true,
	removeComments: true,
	removeRedundantAttributes: true,
	removeScriptTypeAttributes: true,
	removeStyleLinkTypeAttributes: true,
	useShortDoctype: true
};
function viteHtmlPlugin(options = {}) {
	return {
		name: "vben-native-html",
		transformIndexHtml: {
			order: "post",
			async handler(html, ctx) {
				if (!ctx.bundle) return html;
				return await minify(html, {
					...HTML_MINIFY_OPTIONS,
					...options
				});
			}
		}
	};
}
//#endregion
//#region src/plugins/importmap.ts
const DEFAULT_PROVIDER = "jspm.io";
async function getShimsUrl(provide) {
	const version = "1.10.0";
	const shimsSubpath = `dist/es-module-shims.js`;
	const providerShimsMap = {
		"esm.sh": `https://esm.sh/es-module-shims@${version}/${shimsSubpath}`,
		jsdelivr: `https://cdn.jsdelivr.net/npm/es-module-shims@${version}/${shimsSubpath}`,
		"jspm.io": `https://ga.jspm.io/npm:es-module-shims@${version}/${shimsSubpath}`
	};
	return providerShimsMap[provide] || providerShimsMap[DEFAULT_PROVIDER];
}
let generator;
async function viteImportMapPlugin(pluginOptions) {
	const { importmap } = pluginOptions || {};
	let isSSR = false;
	let isBuild = false;
	let installed = false;
	let installError = null;
	const options = Object.assign({}, {
		debug: false,
		defaultProvider: "jspm.io",
		env: [
			"production",
			"browser",
			"module"
		],
		importmap: []
	}, pluginOptions);
	generator = new Generator({
		...options,
		baseUrl: process.cwd()
	});
	if (options?.debug) (async () => {
		for await (const { message, type } of generator.logStream()) console.log(`${type}: ${message}`);
	})();
	const imports = options.inputMap?.imports ?? {};
	const scopes = options.inputMap?.scopes ?? {};
	const firstLayerKeys = Object.keys(scopes);
	const inputMapScopes = [];
	firstLayerKeys.forEach((key) => {
		inputMapScopes.push(...Object.keys(scopes[key] || {}));
	});
	const inputMapImports = Object.keys(imports);
	const allDepNames = [
		...importmap?.map((item) => item.name) || [],
		...inputMapImports,
		...inputMapScopes
	];
	const depNames = new Set(allDepNames);
	const installDeps = importmap?.map((item) => ({
		range: item.range,
		target: item.name
	}));
	return [
		{
			async config(_, { command, isSsrBuild }) {
				isBuild = command === "build";
				isSSR = !!isSsrBuild;
			},
			enforce: "pre",
			name: "importmap:external",
			resolveId(id) {
				if (isSSR || !isBuild) return null;
				if (!depNames.has(id)) return null;
				return {
					external: true,
					id
				};
			}
		},
		{
			enforce: "post",
			name: "importmap:install",
			async resolveId() {
				if (isSSR || !isBuild || installed) return null;
				try {
					installed = true;
					await Promise.allSettled((installDeps || []).map((dep) => generator.install(dep)));
				} catch (error) {
					installError = error;
					installed = false;
				}
				return null;
			}
		},
		{
			buildEnd() {
				if (!installed && !isSSR) {
					installError && console.error(installError);
					throw new Error("Importmap installation failed.");
				}
			},
			enforce: "post",
			name: "importmap:html",
			transformIndexHtml: {
				async handler(html) {
					if (isSSR || !isBuild) return html;
					const importmapJson = generator.getMap();
					if (!importmapJson) return html;
					const esModuleShimsSrc = await getShimsUrl(options.defaultProvider || DEFAULT_PROVIDER);
					html = await minify(await injectShimsToHtml(html, esModuleShimsSrc || "") || html, {
						collapseWhitespace: true,
						minifyCSS: true,
						minifyJS: true,
						removeComments: false
					});
					return {
						html,
						tags: [{
							attrs: { type: "importmap" },
							injectTo: "head-prepend",
							tag: "script",
							children: `${JSON.stringify(importmapJson)}`
						}]
					};
				},
				order: "post"
			}
		}
	];
}
async function injectShimsToHtml(html, esModuleShimUrl) {
	const $ = load(html);
	const $script = $(`script[type='module']`);
	if (!$script) return;
	const entry = $script.attr("src");
	$script.removeAttr("type");
	$script.removeAttr("crossorigin");
	$script.removeAttr("src");
	$script.html(`
if (!HTMLScriptElement.supports || !HTMLScriptElement.supports('importmap')) {
  self.importShim = function () {
      const promise = new Promise((resolve, reject) => {
          document.head.appendChild(
              Object.assign(document.createElement('script'), {
                  src: '${esModuleShimUrl}',
                  crossorigin: 'anonymous',
                  async: true,
                  onload() {
                      if (!importShim.$proxy) {
                          resolve(importShim);
                      } else {
                          reject(new Error('No globalThis.importShim found:' + esModuleShimUrl));
                      }
                  },
                  onerror(error) {
                      reject(error);
                  },
              }),
          );
      });
      importShim.$proxy = true;
      return promise.then((importShim) => importShim(...arguments));
  };
}

var modules = ['${entry}'];
typeof importShim === 'function'
  ? modules.forEach((moduleName) => importShim(moduleName))
  : modules.forEach((moduleName) => import(moduleName));
 `);
	$("body").after($script);
	$("head").remove(`script[type='module']`);
	return $.html();
}
//#endregion
//#region src/plugins/inject-app-loading/index.ts
/**
* 用于生成将loading样式注入到项目中
* 为多app提供loading样式，无需在每个 app -> index.html单独引入
*/
async function viteInjectAppLoadingPlugin(isBuild, env = {}, loadingTemplate = "loading.html") {
	const loadingHtml = await getLoadingRawByHtmlTemplate(loadingTemplate);
	const { version } = await readPackageJSON(process.cwd());
	const envRaw = isBuild ? "prod" : "dev";
	const injectScript = `
  <script data-app-loading="inject-js">
  var theme = localStorage.getItem(${`'${env.VITE_APP_NAMESPACE}-${version}-${envRaw}-preferences-theme'`});
  document.documentElement.classList.toggle('dark', /dark/.test(theme));
<\/script>
`;
	if (!loadingHtml) return;
	return {
		enforce: "pre",
		name: "vite:inject-app-loading",
		transformIndexHtml: {
			handler(html) {
				html = html.replace(/<body\s*>/, `<body>${injectScript}${loadingHtml}`);
				return html;
			},
			order: "pre"
		}
	};
}
/**
* 用于获取loading的html模板
*/
async function getLoadingRawByHtmlTemplate(loadingTemplate) {
	let appLoadingPath = join(process.cwd(), loadingTemplate);
	if (!fs.existsSync(appLoadingPath)) appLoadingPath = join(fileURLToPath(new URL(".", import.meta.url)), "./default-loading.html");
	return await fsp.readFile(appLoadingPath, "utf8");
}
//#endregion
//#region src/plugins/inject-metadata.ts
function resolvePackageVersion(pkgsMeta, name, value, catalog) {
	if (value.includes("catalog:")) return catalog[name];
	if (value.includes("workspace")) return pkgsMeta[name];
	return value;
}
async function resolveMonorepoDependencies() {
	const { packages } = await getPackages();
	const catalog = (await readWorkspaceManifest(findMonorepoRoot()))?.catalog || {};
	const resultDevDependencies = {};
	const resultDependencies = {};
	const pkgsMeta = {};
	for (const { packageJson } of packages) pkgsMeta[packageJson.name] = packageJson.version;
	for (const { packageJson } of packages) {
		const { dependencies = {}, devDependencies = {} } = packageJson;
		for (const [key, value] of Object.entries(dependencies)) resultDependencies[key] = resolvePackageVersion(pkgsMeta, key, value, catalog);
		for (const [key, value] of Object.entries(devDependencies)) resultDevDependencies[key] = resolvePackageVersion(pkgsMeta, key, value, catalog);
	}
	return {
		dependencies: resultDependencies,
		devDependencies: resultDevDependencies
	};
}
/**
* 用于注入项目信息
*/
async function viteMetadataPlugin(root = process.cwd()) {
	const { author, description, homepage, license, version } = await readPackageJSON(root);
	const buildTime = dateUtil().format("YYYY-MM-DD HH:mm:ss");
	return {
		async config() {
			const { dependencies, devDependencies } = await resolveMonorepoDependencies();
			const isAuthorObject = typeof author === "object";
			const authorName = isAuthorObject ? author.name : author;
			const authorEmail = isAuthorObject ? author.email : null;
			const authorUrl = isAuthorObject ? author.url : null;
			return { define: {
				__VBEN_ADMIN_METADATA__: JSON.stringify({
					authorEmail,
					authorName,
					authorUrl,
					buildTime,
					dependencies,
					description,
					devDependencies,
					homepage,
					license,
					version
				}),
				"import.meta.env.VITE_APP_VERSION": JSON.stringify(version)
			} };
		},
		enforce: "post",
		name: "vite:inject-metadata"
	};
}
//#endregion
//#region src/plugins/license.ts
/**
* 用于注入版权信息
* @returns
*/
async function viteLicensePlugin(root = process.cwd()) {
	const { description = "", homepage = "", version = "" } = await readPackageJSON(root);
	return {
		apply: "build",
		enforce: "post",
		generateBundle: {
			handler: (_options, bundle) => {
				const copyrightText = `/*!
  * Vben Admin
  * Version: ${version}
  * Author: vben
  * Copyright (C) 2024 Vben
  * License: MIT License
  * Description: ${description}
  * Date Created: ${dateUtil().format("YYYY-MM-DD ")}
  * Homepage: ${homepage}
  * Contact: ann.vben@gmail.com
*/
              `.trim();
				for (const [, fileContent] of Object.entries(bundle)) if (fileContent.type === "chunk" && fileContent.isEntry) fileContent.code = `${copyrightText}${EOL}${fileContent.code}`;
			},
			order: "post"
		},
		name: "vite:license"
	};
}
//#endregion
//#region src/plugins/nitro-mock.ts
const hmrKeyRe = /^runtimeConfig\.|routeRules\./;
const viteNitroMockPlugin = ({ mockServerPackage = "@vben/backend-mock", port = 5320, verbose = true } = {}) => {
	return {
		async configureServer(server) {
			if (await getPort({ port }) !== port) return;
			const pkg = await getPackage(mockServerPackage);
			if (!pkg) {
				consola.log(`Package ${mockServerPackage} not found. Skip mock server.`);
				return;
			}
			runNitroServer(pkg.dir, port, verbose);
			const _printUrls = server.printUrls;
			server.printUrls = () => {
				_printUrls();
				consola.log(`  ${colors.green("➜")}  ${colors.bold("Nitro Mock Server")}: ${colors.cyan(`http://localhost:${port}/api`)}`);
			};
		},
		enforce: "pre",
		name: "vite:mock-server"
	};
};
async function runNitroServer(rootDir, port, verbose) {
	let nitro;
	const reload = async () => {
		if (nitro) {
			consola.info("Restarting dev server...");
			if ("unwatch" in nitro.options._c12) await nitro.options._c12.unwatch();
			await nitro.close();
		}
		nitro = await createNitro({
			dev: true,
			preset: "nitro-dev",
			rootDir
		}, {
			c12: { async onUpdate({ getDiff, newConfig }) {
				const diff = getDiff();
				if (diff.length === 0) return;
				verbose && consola.info(`Nitro config updated:\n${diff.map((entry) => `  ${entry.toString()}`).join("\n")}`);
				await (diff.every((e) => hmrKeyRe.test(e.key)) ? nitro.updateConfig(newConfig.config) : reload());
			} },
			watch: true
		});
		nitro.hooks.hookOnce("restart", reload);
		await createDevServer(nitro).listen(port, { showURL: false });
		await prepare(nitro);
		await build(nitro);
		if (verbose) {
			console.log("");
			consola.success(colors.bold(colors.green("Nitro Mock Server started.")));
		}
	};
	return await reload();
}
//#endregion
//#region src/plugins/print.ts
const vitePrintPlugin = (options = {}) => {
	const { infoMap = {} } = options;
	return {
		configureServer(server) {
			const _printUrls = server.printUrls;
			server.printUrls = () => {
				_printUrls();
				for (const [key, value] of Object.entries(infoMap)) console.log(`  ${colors.green("➜")}  ${colors.bold(key)}: ${colors.cyan(value)}`);
			};
		},
		enforce: "pre",
		name: "vite:print-info"
	};
};
//#endregion
//#region src/plugins/tailwind-reference.ts
const REFERENCE_LINE = "@reference \"@vben/tailwind-config/theme\";\n";
/**
* Auto-inject @reference into Vue SFC <style> blocks that use @apply.
*
* In Tailwind CSS v4, each Vue SFC <style scoped> block is processed as an
* independent CSS module. If a style block uses @apply with custom theme
* utilities (e.g. bg-primary, text-foreground), it needs access to the
* @theme definition via @reference. This plugin auto-injects it so
* individual components don't need to add it manually.
*/
function viteTailwindReferencePlugin() {
	return {
		enforce: "pre",
		name: "vite:tailwind-reference",
		transform(code, id) {
			if (!id.includes(".vue")) return null;
			if (!id.includes("type=style")) return null;
			if (code.includes("@reference")) return null;
			if (!code.includes("@apply")) return null;
			return {
				code: REFERENCE_LINE + code,
				map: null
			};
		}
	};
}
//#endregion
//#region src/plugins/vxe-table.ts
async function viteVxeTableImportsPlugin() {
	return [lazyImport({ resolvers: [VxeResolver({ libraryName: "vxe-table" }), VxeResolver({ libraryName: "vxe-pc-ui" })] })];
}
//#endregion
//#region src/plugins/index.ts
/**
* 获取条件成立的 vite 插件
* @param conditionPlugins
*/
async function loadConditionPlugins(conditionPlugins) {
	const plugins = [];
	for (const conditionPlugin of conditionPlugins) if (conditionPlugin.condition) {
		const realPlugins = await conditionPlugin.plugins();
		plugins.push(...realPlugins);
	}
	return plugins.flat();
}
/**
* 根据条件获取通用的vite插件
*/
async function loadCommonPlugins(options) {
	const { devtools, injectMetadata, isBuild, visualizer } = options;
	return [
		{
			condition: true,
			plugins: () => [
				viteVue({ script: { defineModel: true } }),
				viteVueJsx(),
				viteTailwindReferencePlugin(),
				tailwindcss()
			]
		},
		{
			condition: !isBuild && devtools,
			plugins: () => [viteVueDevTools()]
		},
		{
			condition: injectMetadata,
			plugins: async () => [await viteMetadataPlugin()]
		},
		{
			condition: isBuild && !!visualizer,
			plugins: () => [viteVisualizerPlugin({
				filename: "./node_modules/.cache/visualizer/stats.html",
				gzipSize: true,
				open: true
			})]
		}
	];
}
/**
* 根据条件获取应用类型的vite插件
*/
async function loadApplicationPlugins(options) {
	const isBuild = options.isBuild;
	const env = options.env;
	const { archiver, archiverPluginOptions, compress, compressTypes, extraAppConfig, html, dayjs, i18n, importmap, importmapOptions, injectAppLoading, license, nitroMock, nitroMockOptions, print, printInfoMap, pwa, pwaOptions, vxeTableLazyImport, ...commonOptions } = options;
	return await loadConditionPlugins([
		...await loadCommonPlugins(commonOptions),
		{
			condition: i18n,
			plugins: async () => {
				return [viteVueI18nPlugin({
					compositionOnly: true,
					fullInstall: true,
					runtimeOnly: true
				})];
			}
		},
		{
			condition: print,
			plugins: async () => {
				return [await vitePrintPlugin({ infoMap: printInfoMap })];
			}
		},
		{
			condition: vxeTableLazyImport,
			plugins: async () => {
				return [await viteVxeTableImportsPlugin()];
			}
		},
		{
			condition: nitroMock,
			plugins: async () => {
				return [await viteNitroMockPlugin(nitroMockOptions)];
			}
		},
		{
			condition: injectAppLoading,
			plugins: async () => [await viteInjectAppLoadingPlugin(!!isBuild, env)]
		},
		{
			condition: license,
			plugins: async () => [await viteLicensePlugin()]
		},
		{
			condition: pwa,
			plugins: () => VitePWA({
				injectRegister: false,
				workbox: { globPatterns: [] },
				...pwaOptions,
				manifest: {
					display: "standalone",
					start_url: "/",
					theme_color: "#ffffff",
					...pwaOptions?.manifest
				}
			})
		},
		{
			condition: isBuild && !!compress,
			plugins: () => {
				const compressPlugins = [];
				if (compressTypes?.includes("brotli")) compressPlugins.push(viteCompressPlugin({
					deleteOriginFile: false,
					ext: ".br"
				}));
				if (compressTypes?.includes("gzip")) compressPlugins.push(viteCompressPlugin({
					deleteOriginFile: false,
					ext: ".gz"
				}));
				return compressPlugins;
			}
		},
		{
			condition: !!html,
			plugins: () => [viteHtmlPlugin(typeof html === "object" ? html : {})]
		},
		{
			condition: isBuild && importmap,
			plugins: () => {
				return [viteImportMapPlugin(importmapOptions)];
			}
		},
		{
			condition: isBuild && extraAppConfig,
			plugins: async () => [await viteExtraAppConfigPlugin({
				isBuild: true,
				root: process.cwd()
			})]
		},
		{
			condition: archiver,
			plugins: async () => {
				return [await viteArchiverPlugin(archiverPluginOptions)];
			}
		},
		{
			condition: dayjs,
			plugins: () => [viteDayjsPlugin()]
		}
	]);
}
/**
* 根据条件获取库类型的vite插件
*/
async function loadLibraryPlugins(options) {
	const isBuild = options.isBuild;
	const { dts, ...commonOptions } = options;
	const dtsOptions = typeof dts === "object" ? dts : void 0;
	return await loadConditionPlugins([...await loadCommonPlugins(commonOptions), {
		condition: isBuild && !!dts,
		plugins: () => [viteDtsPlugin(dtsOptions)]
	}]);
}
//#endregion
//#region src/config/common.ts
async function getCommonConfig() {
	return { build: {
		chunkSizeWarningLimit: 2e3,
		reportCompressedSize: false,
		sourcemap: false
	} };
}
//#endregion
//#region src/config/application.ts
function defineApplicationConfig(userConfigPromise) {
	return defineConfig$1(async (config) => {
		const options = await userConfigPromise?.(config);
		const { appTitle, base, port, ...envConfig } = await loadAndConvertEnv();
		const { command, mode } = config;
		const { application = {}, vite = {} } = options || {};
		const root = process.cwd();
		const isBuild = command === "build";
		const plugins = await loadApplicationPlugins({
			archiver: true,
			archiverPluginOptions: {},
			compress: false,
			compressTypes: ["brotli", "gzip"],
			devtools: true,
			env: loadEnv(mode, root),
			extraAppConfig: true,
			html: true,
			i18n: true,
			importmapOptions: defaultImportmapOptions,
			injectAppLoading: true,
			injectMetadata: true,
			isBuild,
			license: true,
			mode,
			nitroMock: !isBuild,
			nitroMockOptions: {},
			print: !isBuild,
			printInfoMap: { "Vben Admin Docs": "https://doc.vben.pro" },
			pwa: true,
			pwaOptions: getDefaultPwaOptions(appTitle),
			vxeTableLazyImport: true,
			...envConfig,
			...application
		});
		const { injectGlobalScss = true } = application;
		const applicationConfig = {
			base,
			build: {
				rolldownOptions: { output: {
					assetFileNames: "[ext]/[name]-[hash].[ext]",
					chunkFileNames: "js/[name]-[hash].js",
					entryFileNames: "jse/index-[name]-[hash].js",
					minify: isBuild ? { compress: { dropDebugger: true } } : false
				} },
				target: "es2015"
			},
			css: createCssOptions(injectGlobalScss),
			plugins,
			server: {
				host: true,
				port,
				warmup: { clientFiles: [
					"./index.html",
					"./src/bootstrap.ts",
					"./src/{views,layouts,router,store,api,adapter}/*"
				] }
			}
		};
		return mergeConfig(mergeConfig(await getCommonConfig(), applicationConfig), vite);
	});
}
function createCssOptions(injectGlobalScss = true) {
	const root = findMonorepoRoot();
	return { preprocessorOptions: injectGlobalScss ? { scss: {
		additionalData: (content, filepath) => {
			if (relative(root, filepath).startsWith(`apps${path.sep}`)) return `@use "@vben/styles/global" as *;\n${content}`;
			return content;
		},
		importers: [new NodePackageImporter()]
	} } : {} };
}
//#endregion
//#region src/config/library.ts
function defineLibraryConfig(userConfigPromise) {
	return defineConfig$1(async (config) => {
		const options = await userConfigPromise?.(config);
		const { command, mode } = config;
		const { library = {}, vite = {} } = options || {};
		const root = process.cwd();
		const plugins = await loadLibraryPlugins({
			dts: false,
			injectMetadata: true,
			isBuild: command === "build",
			mode,
			...library
		});
		const { dependencies = {}, peerDependencies = {} } = await readPackageJSON(root);
		const externalPackages = [...Object.keys(dependencies), ...Object.keys(peerDependencies)];
		const packageConfig = {
			build: {
				lib: {
					entry: "src/index.ts",
					fileName: () => "index.mjs",
					formats: ["es"]
				},
				rolldownOptions: { external: (id) => {
					return externalPackages.some((pkg) => id === pkg || id.startsWith(`${pkg}/`));
				} }
			},
			plugins
		};
		return mergeConfig(mergeConfig(await getCommonConfig(), packageConfig), vite);
	});
}
//#endregion
//#region src/config/index.ts
function defineConfig(userConfigPromise, type = "auto") {
	let projectType = type;
	if (projectType === "auto") projectType = existsSync(join(process.cwd(), "index.html")) ? "application" : "library";
	switch (projectType) {
		case "application": return defineApplicationConfig(userConfigPromise);
		case "library": return defineLibraryConfig(userConfigPromise);
		default: throw new Error(`Unsupported project type: ${projectType}`);
	}
}
//#endregion
export { defaultImportmapOptions, defineApplicationConfig, defineConfig, defineLibraryConfig, getDefaultPwaOptions, loadAndConvertEnv, loadApplicationPlugins, loadLibraryPlugins, viteArchiverPlugin, viteCompressPlugin, viteDayjsPlugin, viteDtsPlugin, viteHtmlPlugin, viteVisualizerPlugin, viteVxeTableImportsPlugin };
