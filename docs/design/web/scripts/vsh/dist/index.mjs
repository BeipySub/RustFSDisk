import { createRequire } from "node:module";
import { UNICODE, colors, consola, ensureFile, execa, execaCommand, findMonorepoRoot, formatFile, generatorContentHash, getPackages, getStagedFiles, gitAdd, outputJSON, readJSON, toPosixPath } from "@vben/node-utils";
import { cac } from "cac";
import { access, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, extname, join, relative } from "node:path";
import { execSync } from "node:child_process";
import { publint } from "publint";
import { formatMessage } from "publint/utils";
//#region package.json
var version = "5.7.0";
//#endregion
//#region src/check-circular/index.ts
const circularScannerCli = createRequire(import.meta.url).resolve("circular-dependency-scanner/dist/cli.js");
const DEFAULT_CONFIG$1 = {
	allowedExtensions: [
		".cjs",
		".js",
		".jsx",
		".mjs",
		".ts",
		".tsx",
		".vue"
	],
	ignoreDirs: [
		"dist",
		".turbo",
		"output",
		".cache",
		"scripts",
		"internal",
		"packages/effects/request/src/",
		"packages/@core/ui-kit/menu-ui/src/",
		"packages/@core/ui-kit/popup-ui/src/"
	],
	threshold: 0
};
const cache = /* @__PURE__ */ new Map();
async function detectCircularDependencies({ cwd, ignorePattern, staged }) {
	const tempDir = await mkdtemp(join(tmpdir(), "vsh-check-circular-"));
	const outputFile = join(tempDir, "circles.json");
	try {
		const args = [
			circularScannerCli,
			cwd,
			"--output",
			outputFile
		];
		if (staged) args.push("--absolute");
		args.push("--ignore", ignorePattern);
		await execa(process.execPath, args, { cwd });
		await access(outputFile);
		const output = await readFile(outputFile, "utf8");
		return JSON.parse(output);
	} catch (error) {
		if (error?.code === "ENOENT") return [];
		throw error;
	} finally {
		await rm(tempDir, {
			force: true,
			recursive: true
		});
	}
}
/**
* 格式化循环依赖的输出
* @param circles - 循环依赖结果
*/
function formatCircles(circles) {
	if (circles.length === 0) {
		console.log("✅ No circular dependencies found");
		return;
	}
	console.log("⚠️ Circular dependencies found:");
	circles.forEach((circle, index) => {
		console.log(`\nCircular dependency #${index + 1}:`);
		circle.forEach((file) => console.log(`  → ${file}`));
	});
}
/**
* 检查项目中的循环依赖
* @param options - 检查选项
* @param options.staged - 是否只检查暂存区文件
* @param options.verbose - 是否显示详细信息
* @param options.config - 自定义配置
* @returns Promise<void>
*/
async function checkCircular({ config = {}, staged, verbose }) {
	try {
		const finalConfig = {
			...DEFAULT_CONFIG$1,
			...config
		};
		const ignorePattern = `**/{${finalConfig.ignoreDirs.join(",")}}/**`;
		const cacheKey = `${staged}-${process.cwd()}-${ignorePattern}`;
		if (cache.has(cacheKey)) {
			const cachedResults = cache.get(cacheKey);
			if (cachedResults && verbose) formatCircles(cachedResults);
			return;
		}
		const results = await detectCircularDependencies({
			cwd: process.cwd(),
			ignorePattern,
			staged
		});
		if (staged) {
			let files = await getStagedFiles();
			const allowedExtensions = new Set(finalConfig.allowedExtensions);
			files = files.filter((file) => allowedExtensions.has(extname(file)));
			const circularFiles = [];
			for (const file of files) for (const result of results) if (result.flat().includes(file)) circularFiles.push(result);
			cache.set(cacheKey, circularFiles);
			if (verbose) formatCircles(circularFiles);
		} else {
			cache.set(cacheKey, results);
			if (verbose) formatCircles(results);
		}
		if (results.length > 0) console.log("\n⚠️ Warning: Circular dependencies found, please check and fix");
	} catch (error) {
		console.error("❌ Error checking circular dependencies:", error instanceof Error ? error.message : error);
	}
}
/**
* 定义检查循环依赖的命令
* @param cac - CAC实例
*/
function defineCheckCircularCommand(cac) {
	cac.command("check-circular").option("--staged", "Only check staged files").option("--verbose", "Show detailed information").option("--threshold <number>", "Threshold for circular dependencies", { default: 0 }).option("--ignore-dirs <dirs>", "Directories to ignore, comma separated").usage("Analyze project circular dependencies").action(async ({ ignoreDirs, staged, threshold, verbose }) => {
		await checkCircular({
			config: {
				threshold: Number(threshold),
				...ignoreDirs && { ignoreDirs: ignoreDirs.split(",") }
			},
			staged,
			verbose: verbose ?? true
		});
	});
}
//#endregion
//#region src/check-dep/index.ts
const knipCli = join(dirname(createRequire(import.meta.url).resolve("knip")), "..", "bin", "knip.js");
const DEFAULT_CONFIG = {
	ignore: [
		"dist/**",
		"docs/**",
		"node_modules/**",
		"public/**"
	],
	ignoreBinaries: [],
	ignoreDependencies: [
		"@iconify/json",
		"@vben-core/design",
		"@vben/commitlint-config",
		"@vben/eslint-config",
		"@vben/stylelint-config",
		"@vben/tailwind-config",
		"@vben/vite-config",
		"@vben/oxlint-config",
		"playwright",
		"rimraf",
		"tailwindcss"
	],
	ignoreWorkspaces: ["internal/lint-configs/*", "scripts/*"]
};
/**
* 格式化依赖检查结果
* @param result - 依赖检查结果
*/
function formatResult(result) {
	let hasIssues = false;
	for (const issue of result.issues) {
		const hasDeps = issue.dependencies.length > 0;
		const hasDevDeps = issue.devDependencies.length > 0;
		if (!hasDeps && !hasDevDeps) continue;
		hasIssues = true;
		console.log(`\n📦 ${issue.file}`);
		if (hasDeps) {
			console.log("⚠️ Unused dependencies:");
			for (const dep of issue.dependencies) console.log(`  - ${dep.name}`);
		}
		if (hasDevDeps) {
			console.log("⚠️ Unused devDependencies:");
			for (const dep of issue.devDependencies) console.log(`  - ${dep.name}`);
		}
	}
	if (!hasIssues) console.log("\n✅ Dependency check completed, no issues found");
}
/**
* 运行依赖检查
*/
async function runKnipCheck() {
	const cwd = process.cwd();
	const tempDir = await mkdtemp(join(tmpdir(), "vsh-check-dep-"));
	const configFile = join(tempDir, "knip.json");
	try {
		await writeFile(configFile, JSON.stringify(DEFAULT_CONFIG));
		const args = [
			knipCli,
			"--config",
			configFile,
			"--include",
			"dependencies",
			"--reporter",
			"json",
			"--no-config-hints"
		];
		await execa(process.execPath, args, { cwd });
		console.log("\n✅ Dependency check completed, no issues found");
	} catch (error) {
		const execaError = error;
		if (execaError.exitCode === 1 && execaError.stdout) {
			formatResult(JSON.parse(execaError.stdout));
			return;
		}
		console.error("❌ Dependency check failed:", error instanceof Error ? error.message : error);
	} finally {
		await rm(tempDir, {
			force: true,
			recursive: true
		});
	}
}
/**
* 定义依赖检查命令
* @param cac - CAC实例
*/
function defineCheckDepCommand(cac) {
	cac.command("check-dep").usage("Analyze project dependencies using knip").action(async () => {
		await runKnipCheck();
	});
}
//#endregion
//#region src/code-workspace/index.ts
const CODE_WORKSPACE_FILE = join("vben-admin.code-workspace");
async function createCodeWorkspace({ autoCommit = false, spaces = 2 }) {
	const { packages, rootDir } = await getPackages();
	let folders = packages.map((pkg) => {
		const { dir, packageJson } = pkg;
		return {
			name: packageJson.name,
			path: toPosixPath(relative(rootDir, dir))
		};
	});
	folders = folders.filter(Boolean);
	const monorepoRoot = findMonorepoRoot();
	const outputPath = join(monorepoRoot, CODE_WORKSPACE_FILE);
	await outputJSON(outputPath, { folders }, spaces);
	await formatFile(outputPath);
	if (autoCommit) await gitAdd(CODE_WORKSPACE_FILE, monorepoRoot);
}
async function runCodeWorkspace({ autoCommit, spaces }) {
	await createCodeWorkspace({
		autoCommit,
		spaces
	});
	if (autoCommit) return;
	consola.log("");
	consola.success(colors.green(`${CODE_WORKSPACE_FILE} is updated!`));
	consola.log("");
}
function defineCodeWorkspaceCommand(cac) {
	cac.command("code-workspace").usage("Update the `.code-workspace` file").option("--spaces [number]", ".code-workspace JSON file spaces.", { default: 2 }).option("--auto-commit", "auto commit .code-workspace JSON file.", { default: false }).action(runCodeWorkspace);
}
//#endregion
//#region src/lint/index.ts
async function runLint({ format, threads }) {
	const threadsArg = threads ? ` --threads=${threads}` : ` --threads=2`;
	if (format) {
		await execaCommand(`stylelint "**/*.{vue,css,less,scss}" --cache --fix`, { stdio: "inherit" });
		await execaCommand(`oxfmt${threadsArg}`, { stdio: "inherit" });
		await execaCommand(`oxlint --fix${threadsArg}`, { stdio: "inherit" });
		await execaCommand(`eslint . --cache --fix`, { stdio: "inherit" });
		return;
	}
	const subprocesses = [
		execaCommand(`oxfmt --check${threadsArg}`, { stdio: "inherit" }),
		execaCommand(`oxlint${threadsArg}`, { stdio: "inherit" }),
		execaCommand(`eslint . --cache`, { stdio: "inherit" }),
		execaCommand(`stylelint "**/*.{vue,css,less,scss}" --cache`, { stdio: "inherit" })
	];
	try {
		await Promise.all(subprocesses);
	} catch (error) {
		for (const subprocess of subprocesses) try {
			if (process.platform === "win32" && subprocess.pid) execSync(`taskkill /F /T /PID ${subprocess.pid}`, { stdio: "ignore" });
			else subprocess.kill("SIGKILL");
		} catch {}
		await Promise.allSettled(subprocesses);
		throw error;
	}
}
function defineLintCommand(cac) {
	cac.command("lint").usage("Batch execute project lint check.").option("--format", "Format lint problem.").option("--threads <count>", "Number of threads for oxfmt and oxlint.").action(runLint);
}
//#endregion
//#region src/publint/index.ts
const CACHE_FILE = join("node_modules", ".cache", "publint", ".pkglintcache.json");
/**
* Get files that require lint
* @param files
*/
async function getLintFiles(files = []) {
	const lintFiles = [];
	if (files?.length > 0) return files.filter((file) => basename(file) === "package.json");
	const { packages } = await getPackages();
	for (const { dir } of packages) lintFiles.push(join(dir, "package.json"));
	return lintFiles;
}
function getCacheFile() {
	return join(findMonorepoRoot(), CACHE_FILE);
}
async function readCache(cacheFile) {
	try {
		await ensureFile(cacheFile);
		return await readJSON(cacheFile);
	} catch {
		return {};
	}
}
async function runPublint(files, { check }) {
	const lintFiles = await getLintFiles(files);
	const cacheFile = getCacheFile();
	const cache = await readCache(cacheFile);
	const results = await Promise.all(lintFiles.map(async (file) => {
		try {
			const pkgJson = await readJSON(file);
			if (pkgJson.private) return null;
			Reflect.deleteProperty(pkgJson, "dependencies");
			Reflect.deleteProperty(pkgJson, "devDependencies");
			Reflect.deleteProperty(pkgJson, "peerDependencies");
			const hash = generatorContentHash(JSON.stringify(pkgJson));
			const publintResult = cache?.[file]?.hash === hash ? cache?.[file]?.result ?? [] : await publint({
				level: "suggestion",
				pkgDir: dirname(file),
				strict: true
			});
			cache[file] = {
				hash,
				result: publintResult
			};
			return {
				pkgJson,
				pkgPath: file,
				publintResult
			};
		} catch {
			return null;
		}
	}));
	await outputJSON(cacheFile, cache);
	printResult(results, check);
}
function printResult(results, check) {
	let errorCount = 0;
	let warningCount = 0;
	let suggestionsCount = 0;
	for (const result of results) {
		if (!result) continue;
		const { pkgJson, pkgPath, publintResult } = result;
		const messages = publintResult?.messages ?? [];
		if (messages?.length < 1) continue;
		consola.log("");
		consola.log(pkgPath);
		for (const message of messages) {
			switch (message.type) {
				case "error":
					errorCount++;
					break;
				case "suggestion":
					suggestionsCount++;
					break;
				case "warning":
					warningCount++;
					break;
			}
			const ruleUrl = `https://publint.dev/rules#${message.code.toLocaleLowerCase()}`;
			consola.log(`  ${formatMessage(message, pkgJson)}${colors.dim(` ${ruleUrl}`)}`);
		}
	}
	const totalCount = warningCount + errorCount + suggestionsCount;
	if (totalCount > 0) {
		consola.error(colors.red(`${UNICODE.FAILURE} ${totalCount} problem (${errorCount} errors, ${warningCount} warnings, ${suggestionsCount} suggestions)`));
		!check && process.exit(1);
	} else consola.log(colors.green(`${UNICODE.SUCCESS} No problem`));
}
function definePubLintCommand(cac) {
	cac.command("publint [...files]").usage("Check if the monorepo package conforms to the publint standard.").option("--check", "Only errors are checked, no program exit is performed.").action(runPublint);
}
//#endregion
//#region src/index.ts
const COMMAND_DESCRIPTIONS = {
	"check-circular": "Check for circular dependencies",
	"check-dep": "Check for unused dependencies",
	"code-workspace": "Manage VS Code workspace settings",
	lint: "Run linting on the project",
	publint: "Check package.json files for publishing standards"
};
/**
* Initialize and run the CLI
*/
async function main() {
	try {
		const vsh = cac("vsh");
		defineLintCommand(vsh);
		definePubLintCommand(vsh);
		defineCodeWorkspaceCommand(vsh);
		defineCheckCircularCommand(vsh);
		defineCheckDepCommand(vsh);
		vsh.usage("vsh <command> [options]");
		vsh.help();
		vsh.version(version);
		vsh.parse(void 0, { run: false });
		if (!vsh.matchedCommand && vsh.args.length > 0) {
			const unknownCmd = String(vsh.args[0]);
			consola.error(colors.red(`Invalid command: ${unknownCmd}`), "\n", colors.yellow("Available commands:"), "\n", Object.entries(COMMAND_DESCRIPTIONS).map(([name, desc]) => `  ${colors.cyan(name)} - ${desc}`).join("\n"));
			process.exit(1);
		}
		await vsh.runMatchedCommand();
	} catch (error) {
		consola.error(colors.red("An unexpected error occurred:"), "\n", error instanceof Error ? error.message : error);
		process.exit(1);
	}
}
main().catch((error) => {
	consola.error(colors.red("Failed to start CLI:"), "\n", error instanceof Error ? error.message : error);
	process.exit(1);
});
//#endregion
export {};
