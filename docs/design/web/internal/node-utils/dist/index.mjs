import dayjs from "dayjs";
import timezone from "dayjs/plugin/timezone.js";
import utc from "dayjs/plugin/utc.js";
import fs, { default as fs$1 } from "node:fs/promises";
import { execa } from "execa";
import { existsSync, promises } from "node:fs";
import path, { dirname, join, posix, resolve } from "node:path";
import { createHash } from "node:crypto";
import * as manypkg from "@manypkg/get-packages";
import ora from "ora";
import colors from "chalk";
import { consola } from "consola";
import { readPackageJSON } from "pkg-types";
import { rimraf } from "rimraf";
export * from "execa";
export * from "@changesets/git";
//#region \0rolldown/runtime.js
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __exportAll = (all, no_symbols) => {
	let target = {};
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
	if (!no_symbols) __defProp(target, Symbol.toStringTag, { value: "Module" });
	return target;
};
var __copyProps = (to, from, except, desc) => {
	if (from && typeof from === "object" || typeof from === "function") for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
		key = keys[i];
		if (!__hasOwnProp.call(to, key) && key !== except) __defProp(to, key, {
			get: ((k) => from[k]).bind(null, key),
			enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
		});
	}
	return to;
};
var __reExport = (target, mod, secondTarget) => (__copyProps(target, mod, "default"), secondTarget && __copyProps(secondTarget, mod, "default"));
//#endregion
//#region src/constants.ts
var UNICODE = /* @__PURE__ */ function(UNICODE) {
	UNICODE["FAILURE"] = "✖";
	UNICODE["SUCCESS"] = "✔";
	return UNICODE;
}(UNICODE || {});
//#endregion
//#region src/date.ts
dayjs.extend(utc);
dayjs.extend(timezone);
dayjs.tz.setDefault("Asia/Shanghai");
const dateUtil = dayjs;
//#endregion
//#region src/formatter.ts
async function formatFile(filepath) {
	await execa("oxfmt", [filepath], { stdio: "inherit" });
	return await fs$1.readFile(filepath, "utf8");
}
//#endregion
//#region src/fs.ts
async function outputJSON(filePath, data, spaces = 2) {
	try {
		const dir = dirname(filePath);
		await promises.mkdir(dir, { recursive: true });
		const jsonData = JSON.stringify(data, null, spaces);
		await promises.writeFile(filePath, jsonData, "utf8");
	} catch (error) {
		console.error("Error writing JSON file:", error);
		throw error;
	}
}
async function ensureFile(filePath) {
	try {
		const dir = dirname(filePath);
		await promises.mkdir(dir, { recursive: true });
		await promises.writeFile(filePath, "", { flag: "a" });
	} catch (error) {
		console.error("Error ensuring file:", error);
		throw error;
	}
}
async function readJSON(filePath) {
	try {
		const data = await promises.readFile(filePath, "utf8");
		return JSON.parse(data);
	} catch (error) {
		console.error("Error reading JSON file:", error);
		throw error;
	}
}
//#endregion
//#region src/git.ts
var git_exports = /* @__PURE__ */ __exportAll({ getStagedFiles: () => getStagedFiles });
import * as import__changesets_git from "@changesets/git";
__reExport(git_exports, import__changesets_git);
/**
* 获取暂存区文件
*/
async function getStagedFiles() {
	try {
		const { stdout } = await execa("git", [
			"-c",
			"submodule.recurse=false",
			"diff",
			"--staged",
			"--diff-filter=ACMR",
			"--name-only",
			"--ignore-submodules",
			"-z"
		]);
		let changedList = stdout ? stdout.replace(/\0$/, "").split("\0") : [];
		changedList = changedList.map((item) => path.resolve(process.cwd(), item));
		const changedSet = new Set(changedList);
		changedSet.delete("");
		return [...changedSet];
	} catch (error) {
		console.error("Failed to get staged files:", error);
		return [];
	}
}
//#endregion
//#region src/hash.ts
/**
* 生产基于内容的 hash，可自定义长度
* @param content
* @param hashLSize
*/
function generatorContentHash(content, hashLSize) {
	const hash = createHash("md5").update(content, "utf8").digest("hex");
	if (hashLSize) return hash.slice(0, hashLSize);
	return hash;
}
//#endregion
//#region src/monorepo.ts
const { getPackages: getPackagesFunc, getPackagesSync: getPackagesSyncFunc } = manypkg;
/**
* 查找大仓的根目录
* @param cwd
*/
function findMonorepoRoot(cwd = process.cwd()) {
	let currentDir = resolve(cwd);
	while (true) {
		if (existsSync(join(currentDir, "pnpm-lock.yaml"))) return currentDir;
		const parentDir = dirname(currentDir);
		if (parentDir === currentDir) return "";
		currentDir = parentDir;
	}
}
/**
* 获取大仓的所有包
*/
function getPackagesSync() {
	return getPackagesSyncFunc(findMonorepoRoot());
}
/**
* 获取大仓的所有包
*/
async function getPackages() {
	return await getPackagesFunc(findMonorepoRoot());
}
/**
* 获取大仓指定的包
*/
async function getPackage(pkgName) {
	const { packages } = await getPackages();
	return packages.find((pkg) => pkg.packageJson.name === pkgName);
}
//#endregion
//#region src/path.ts
/**
* 将给定的文件路径转换为 POSIX 风格。
* @param {string} pathname - 原始文件路径。
*/
function toPosixPath(pathname) {
	return pathname.split(`\\`).join(posix.sep);
}
//#endregion
//#region src/spinner.ts
async function spinner({ failedText, successText, title }, callback) {
	const loading = ora(title).start();
	try {
		const result = await callback();
		loading.succeed(successText || "Success!");
		return result;
	} catch (error) {
		loading.fail(failedText || "Failed!");
		throw error;
	} finally {
		loading.stop();
	}
}
__reExport(/* @__PURE__ */ __exportAll({
	UNICODE: () => UNICODE,
	colors: () => colors,
	consola: () => consola,
	dateUtil: () => dateUtil,
	ensureFile: () => ensureFile,
	findMonorepoRoot: () => findMonorepoRoot,
	formatFile: () => formatFile,
	fs: () => fs,
	generatorContentHash: () => generatorContentHash,
	getPackage: () => getPackage,
	getPackages: () => getPackages,
	getPackagesSync: () => getPackagesSync,
	getStagedFiles: () => getStagedFiles,
	gitAdd: () => git_exports.add,
	outputJSON: () => outputJSON,
	readJSON: () => readJSON,
	readPackageJSON: () => readPackageJSON,
	rimraf: () => rimraf,
	spinner: () => spinner,
	toPosixPath: () => toPosixPath
}), git_exports);
//#endregion
var gitAdd = git_exports.add;
export { UNICODE, colors, consola, dateUtil, ensureFile, findMonorepoRoot, formatFile, fs, generatorContentHash, getPackage, getPackages, getPackagesSync, getStagedFiles, gitAdd, outputJSON, readJSON, readPackageJSON, rimraf, spinner, toPosixPath };
