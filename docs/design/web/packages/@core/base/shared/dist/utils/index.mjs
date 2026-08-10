import { clsx } from "clsx";
import { twMerge } from "tailwind-merge";
import dayjs from "dayjs";
import timezone from "dayjs/plugin/timezone.js";
import utc from "dayjs/plugin/utc.js";
import { isFunction, isObject, isString } from "@vue/shared";
import { createDefu, createDefu as createMerge, defu as merge } from "defu";
import { get, isEqual, set } from "es-toolkit/compat";
import cloneDeep from "lodash.clonedeep";
//#region src/utils/cn.ts
function cn(...inputs) {
	return twMerge(clsx(inputs));
}
//#endregion
//#region src/utils/date.ts
dayjs.extend(utc);
dayjs.extend(timezone);
function formatDate(time, format = "YYYY-MM-DD") {
	if (time === void 0 || time === null || time === "") return "";
	try {
		const date = dayjs.isDayjs(time) ? time : dayjs(time);
		if (!date.isValid()) throw new Error("Invalid date");
		return date.tz().format(format);
	} catch (error) {
		console.error(`Error formatting date: ${error}`);
		return String(time ?? "");
	}
}
function formatDateTime(time) {
	return formatDate(time, "YYYY-MM-DD HH:mm:ss");
}
function isDate(value) {
	return value instanceof Date;
}
function isDayjsObject(value) {
	return dayjs.isDayjs(value);
}
/**
* 获取当前时区
* @returns 当前时区
*/
const getSystemTimezone = () => {
	return dayjs.tz.guess();
};
/**
* 自定义设置的时区
*/
let currentTimezone = getSystemTimezone();
/**
* 设置默认时区
* @param timezone
*/
const setCurrentTimezone = (timezone) => {
	currentTimezone = timezone || getSystemTimezone();
	dayjs.tz.setDefault(currentTimezone);
};
/**
* 获取设置的时区
* @returns 设置的时区
*/
const getCurrentTimezone = () => {
	return currentTimezone;
};
//#endregion
//#region src/utils/diff.ts
function arraysEqual(a, b) {
	if (a.length !== b.length) return false;
	const counter = /* @__PURE__ */ new Map();
	for (const value of a) counter.set(value, (counter.get(value) || 0) + 1);
	for (const value of b) {
		const count = counter.get(value);
		if (count === void 0 || count === 0) return false;
		counter.set(value, count - 1);
	}
	return true;
}
function diff(obj1, obj2) {
	function findDifferences(o1, o2) {
		if (Array.isArray(o1) && Array.isArray(o2)) {
			if (!arraysEqual(o1, o2)) return o2;
			return;
		}
		if (typeof o1 === "object" && typeof o2 === "object" && o1 !== null && o2 !== null) {
			const diffResult = {};
			new Set([...Object.keys(o1), ...Object.keys(o2)]).forEach((key) => {
				const valueDiff = findDifferences(o1[key], o2[key]);
				if (valueDiff !== void 0) diffResult[key] = valueDiff;
			});
			return Object.keys(diffResult).length > 0 ? diffResult : void 0;
		}
		return o1 === o2 ? void 0 : o2;
	}
	return findDifferences(obj1, obj2);
}
//#endregion
//#region src/utils/dom.ts
/**
* 获取元素可见信息
* @param element
*/
function getElementVisibleRect(element) {
	if (!element) return {
		bottom: 0,
		height: 0,
		left: 0,
		right: 0,
		top: 0,
		width: 0
	};
	const rect = element.getBoundingClientRect();
	const viewHeight = Math.max(document.documentElement.clientHeight, window.innerHeight);
	const top = Math.max(rect.top, 0);
	const bottom = Math.min(rect.bottom, viewHeight);
	const viewWidth = Math.max(document.documentElement.clientWidth, window.innerWidth);
	const left = Math.max(rect.left, 0);
	const right = Math.min(rect.right, viewWidth);
	if (top >= viewHeight || bottom <= 0 || left >= viewWidth || right <= 0) return {
		bottom: 0,
		height: 0,
		left: 0,
		right: 0,
		top: 0,
		width: 0
	};
	return {
		bottom,
		height: Math.max(0, bottom - top),
		left,
		right,
		top,
		width: Math.max(0, right - left)
	};
}
function getScrollbarWidth() {
	const scrollDiv = document.createElement("div");
	scrollDiv.style.visibility = "hidden";
	scrollDiv.style.overflow = "scroll";
	scrollDiv.style.position = "absolute";
	scrollDiv.style.top = "-9999px";
	document.body.append(scrollDiv);
	const innerDiv = document.createElement("div");
	scrollDiv.append(innerDiv);
	const scrollbarWidth = scrollDiv.offsetWidth - innerDiv.offsetWidth;
	scrollDiv.remove();
	return scrollbarWidth;
}
function needsScrollbar() {
	const doc = document.documentElement;
	const body = document.body;
	const overflowY = window.getComputedStyle(body).overflowY;
	if (overflowY === "scroll" || overflowY === "auto") return doc.scrollHeight > window.innerHeight;
	return doc.scrollHeight > window.innerHeight;
}
function triggerWindowResize() {
	const resizeEvent = new Event("resize");
	window.dispatchEvent(resizeEvent);
}
//#endregion
//#region src/utils/window.ts
/**
* 新窗口打开URL。
*
* @param url - 需要打开的网址。
* @param options - 打开窗口的选项。
*/
function openWindow(url, options = {}) {
	const { noopener = true, noreferrer = true, target = "_blank" } = options;
	const features = [noopener && "noopener=yes", noreferrer && "noreferrer=yes"].filter(Boolean).join(",");
	window.open(url, target, features);
}
/**
* 在新窗口中打开路由。
* @param path
*/
function openRouteInNewWindow(path) {
	const { hash, origin } = location;
	const fullPath = path.startsWith("/") ? path : `/${path}`;
	openWindow(`${origin}${hash && !fullPath.startsWith("/#") ? "/#" : ""}${fullPath}`, { target: "_blank" });
}
//#endregion
//#region src/utils/download.ts
const DEFAULT_FILENAME = "downloaded_file";
/**
* 通过 URL 下载文件，支持跨域
* @throws {Error} - 当下载失败时抛出错误
*/
async function downloadFileFromUrl({ fileName, source, target = "_blank" }) {
	if (!source || typeof source !== "string") throw new Error("Invalid URL.");
	const isChrome = window.navigator.userAgent.toLowerCase().includes("chrome");
	const isSafari = window.navigator.userAgent.toLowerCase().includes("safari");
	if (/iP/.test(window.navigator.userAgent)) {
		console.error("Your browser does not support download!");
		return;
	}
	if (isChrome || isSafari) {
		triggerDownload(source, resolveFileName(source, fileName));
		return;
	}
	if (!source.includes("?")) source += "?download";
	openWindow(source, { target });
}
/**
* 通过 Base64 下载文件
*/
function downloadFileFromBase64({ fileName, source }) {
	if (!source || typeof source !== "string") throw new Error("Invalid Base64 data.");
	triggerDownload(source, fileName || DEFAULT_FILENAME);
}
/**
* 通过图片 URL 下载图片文件
*/
async function downloadFileFromImageUrl({ fileName, source }) {
	downloadFileFromBase64({
		fileName,
		source: await urlToBase64(source)
	});
}
/**
* 通过 Blob 下载文件
*/
function downloadFileFromBlob({ fileName = DEFAULT_FILENAME, source }) {
	if (!(source instanceof Blob)) throw new TypeError("Invalid Blob data.");
	triggerDownload(URL.createObjectURL(source), fileName);
}
/**
* 下载文件，支持 Blob、字符串和其他 BlobPart 类型
*/
function downloadFileFromBlobPart({ fileName = DEFAULT_FILENAME, source }) {
	const blob = source instanceof Blob ? source : new Blob([source], { type: "application/octet-stream" });
	triggerDownload(URL.createObjectURL(blob), fileName);
}
/**
* img url to base64
* @param url
*/
function urlToBase64(url, mineType) {
	return new Promise((resolve, reject) => {
		let canvas = document.createElement("CANVAS");
		const ctx = canvas?.getContext("2d");
		const img = new Image();
		img.crossOrigin = "";
		img.addEventListener("load", () => {
			if (!canvas || !ctx) return reject(/* @__PURE__ */ new Error("Failed to create canvas."));
			canvas.height = img.height;
			canvas.width = img.width;
			ctx.drawImage(img, 0, 0);
			const dataURL = canvas.toDataURL(mineType || "image/png");
			canvas = null;
			resolve(dataURL);
		});
		img.src = url;
	});
}
/**
* 通用下载触发函数
* @param href - 文件下载的 URL
* @param fileName - 下载文件的名称，如果未提供则自动识别
* @param revokeDelay - 清理 URL 的延迟时间 (毫秒)
*/
function triggerDownload(href, fileName, revokeDelay = 100) {
	const finalFileName = fileName || "downloaded_file";
	const link = document.createElement("a");
	link.href = href;
	link.download = finalFileName;
	link.style.display = "none";
	if (link.download === void 0) link.setAttribute("target", "_blank");
	document.body.append(link);
	link.click();
	link.remove();
	setTimeout(() => URL.revokeObjectURL(href), revokeDelay);
}
function resolveFileName(url, fileName) {
	return fileName || url.slice(url.lastIndexOf("/") + 1) || DEFAULT_FILENAME;
}
//#endregion
//#region src/utils/inference.ts
/**
* 检查传入的值是否为undefined。
*
* @param {unknown} value 要检查的值。
* @returns {boolean} 如果值是undefined，返回true，否则返回false。
*/
function isUndefined(value) {
	return value === void 0;
}
/**
* 检查传入的值是否为boolean
* @param value
* @returns 如果值是布尔值，返回true，否则返回false。
*/
function isBoolean(value) {
	return typeof value === "boolean";
}
/**
* 检查传入的值是否为空。
*
* 以下情况将被认为是空：
* - 值为null。
* - 值为undefined。
* - 值为一个空字符串。
* - 值为一个长度为0的数组。
* - 值为一个没有元素的Map或Set。
* - 值为一个没有属性的对象。
*
* @param {T} value 要检查的值。
* @returns {boolean} 如果值为空，返回true，否则返回false。
*/
function isEmpty(value) {
	if (value === null || value === void 0) return true;
	if (Array.isArray(value) || isString(value)) return value.length === 0;
	if (value instanceof Map || value instanceof Set) return value.size === 0;
	if (isObject(value)) return Object.keys(value).length === 0;
	return false;
}
/**
* 检查传入的字符串是否为有效的HTTP或HTTPS URL。
*
* @param {string} url 要检查的字符串。
* @return {boolean} 如果字符串是有效的HTTP或HTTPS URL，返回true，否则返回false。
*/
function isHttpUrl(url) {
	if (!url) return false;
	return /^https?:\/\/.*$/.test(url);
}
/**
* 检查传入的值是否为window对象。
*
* @param {any} value 要检查的值。
* @returns {boolean} 如果值是window对象，返回true，否则返回false。
*/
function isWindow(value) {
	return typeof window !== "undefined" && value !== null && value === value.window;
}
/**
* 检查当前运行环境是否为Mac OS。
*
* 这个函数通过检查navigator.userAgent字符串来判断当前运行环境。
* 如果userAgent字符串中包含"macintosh"或"mac os x"（不区分大小写），则认为当前环境是Mac OS。
*
* @returns {boolean} 如果当前环境是Mac OS，返回true，否则返回false。
*/
function isMacOs() {
	return /macintosh|mac os x/i.test(navigator.userAgent);
}
/**
* 检查当前运行环境是否为Windows OS。
*
* 这个函数通过检查navigator.userAgent字符串来判断当前运行环境。
* 如果userAgent字符串中包含"windows"或"win32"（不区分大小写），则认为当前环境是Windows OS。
*
* @returns {boolean} 如果当前环境是Windows OS，返回true，否则返回false。
*/
function isWindowsOs() {
	return /windows|win32/i.test(navigator.userAgent);
}
/**
* 检查传入的值是否为数字
* @param value
*/
function isNumber(value) {
	return typeof value === "number" && Number.isFinite(value);
}
/**
* Returns the first value in the provided list that is neither `null` nor `undefined`.
*
* This function iterates over the input values and returns the first one that is
* not strictly equal to `null` or `undefined`. If all values are either `null` or
* `undefined`, it returns `undefined`.
*
* @template T - The type of the input values.
* @param {...(T | null | undefined)[]} values - A list of values to evaluate.
* @returns {T | undefined} - The first value that is not `null` or `undefined`, or `undefined` if none are found.
*
* @example
* // Returns 42 because it is the first non-null, non-undefined value.
* getFirstNonNullOrUndefined(undefined, null, 42, 'hello'); // 42
*
* @example
* // Returns 'hello' because it is the first non-null, non-undefined value.
* getFirstNonNullOrUndefined(null, undefined, 'hello', 123); // 'hello'
*
* @example
* // Returns undefined because all values are either null or undefined.
* getFirstNonNullOrUndefined(undefined, null); // undefined
*/
function getFirstNonNullOrUndefined(...values) {
	for (const value of values) if (value !== void 0 && value !== null) return value;
}
//#endregion
//#region src/utils/letter.ts
/**
* 将字符串的首字母大写
* @param string
*/
function capitalizeFirstLetter(string) {
	return string.charAt(0).toUpperCase() + string.slice(1);
}
/**
* 将字符串的首字母转换为小写。
*
* @param str 要转换的字符串
* @returns 首字母小写的字符串
*/
function toLowerCaseFirstLetter(str) {
	if (!str) return str;
	return str.charAt(0).toLowerCase() + str.slice(1);
}
/**
*  生成驼峰命名法的键名
* @param key
* @param parentKey
*/
function toCamelCase(key, parentKey) {
	if (!parentKey) return key;
	return parentKey + key.charAt(0).toUpperCase() + key.slice(1);
}
function kebabToCamelCase(str) {
	return str.split("-").filter(Boolean).map((word, index) => index === 0 ? word : word.charAt(0).toUpperCase() + word.slice(1)).join("");
}
//#endregion
//#region src/utils/merge.ts
const mergeWithArrayOverride = createDefu((originObj, key, updates) => {
	if (Array.isArray(originObj[key]) && Array.isArray(updates)) {
		originObj[key] = updates;
		return true;
	}
});
//#endregion
//#region src/utils/nprogress.ts
let nProgressInstance = null;
/**
* 动态加载NProgress库，并进行配置。
* 此函数首先检查是否已经加载过NProgress库，如果已经加载过，则直接返回NProgress实例。
* 否则，动态导入NProgress库，进行配置，然后返回NProgress实例。
*
* @returns  NProgress实例的Promise对象。
*/
async function loadNprogress() {
	if (nProgressInstance) return nProgressInstance;
	nProgressInstance = await import("nprogress");
	nProgressInstance.configure({
		showSpinner: true,
		speed: 300
	});
	return nProgressInstance;
}
/**
* 开始显示进度条。
* 此函数首先加载NProgress库，然后调用NProgress的start方法开始显示进度条。
*/
async function startProgress() {
	(await loadNprogress())?.start();
}
/**
* 停止显示进度条，并隐藏进度条。
* 此函数首先加载NProgress库，然后调用NProgress的done方法停止并隐藏进度条。
*/
async function stopProgress() {
	(await loadNprogress())?.done();
}
//#endregion
//#region src/utils/resources.ts
/**
* 加载js文件
* @param src js文件地址
*/
function loadScript(src) {
	return new Promise((resolve, reject) => {
		if (document.querySelector(`script[src="${src}"]`)) return resolve();
		const script = document.createElement("script");
		script.src = src;
		script.addEventListener("load", () => resolve());
		script.addEventListener("error", () => reject(/* @__PURE__ */ new Error(`Failed to load script: ${src}`)));
		document.head.append(script);
	});
}
//#endregion
//#region src/utils/stack.ts
/**
* @zh_CN 栈数据结构
*/
var Stack = class {
	/**
	* @zh_CN 栈内元素数量
	*/
	get size() {
		return this.items.length;
	}
	/**
	* @zh_CN 是否去重
	*/
	dedup;
	/**
	* @zh_CN 栈内元素
	*/
	items = [];
	/**
	* @zh_CN 栈的最大容量
	*/
	maxSize;
	constructor(dedup = true, maxSize) {
		this.maxSize = maxSize;
		this.dedup = dedup;
	}
	/**
	* @zh_CN 清空栈内元素
	*/
	clear() {
		this.items.length = 0;
	}
	/**
	* @zh_CN 查看栈顶元素
	* @returns 栈顶元素
	*/
	peek() {
		return this.items[this.items.length - 1];
	}
	/**
	* @zh_CN 出栈
	* @returns 栈顶元素
	*/
	pop() {
		return this.items.pop();
	}
	/**
	* @zh_CN 入栈
	* @param items 要入栈的元素
	*/
	push(...items) {
		items.forEach((item) => {
			if (this.dedup) {
				const index = this.items.indexOf(item);
				if (index !== -1) this.items.splice(index, 1);
			}
			this.items.push(item);
			if (this.maxSize && this.items.length > this.maxSize) this.items.splice(0, this.items.length - this.maxSize);
		});
	}
	/**
	* @zh_CN 移除栈内元素
	* @param itemList 要移除的元素列表
	*/
	remove(...itemList) {
		this.items = this.items.filter((i) => !itemList.includes(i));
	}
	/**
	* @zh_CN 保留栈内元素
	* @param itemList 要保留的元素列表
	*/
	retain(itemList) {
		this.items = this.items.filter((i) => itemList.includes(i));
	}
	/**
	* @zh_CN 转换为数组
	* @returns 栈内元素数组
	*/
	toArray() {
		return [...this.items];
	}
};
/**
* @zh_CN 创建一个栈实例
* @param dedup 是否去重
* @param maxSize 栈的最大容量
* @returns 栈实例
*/
const createStack = (dedup = true, maxSize) => new Stack(dedup, maxSize);
//#endregion
//#region src/utils/state-handler.ts
var StateHandler = class {
	condition = false;
	rejectCondition = null;
	resolveCondition = null;
	isConditionTrue() {
		return this.condition;
	}
	reset() {
		this.condition = false;
		this.clearPromises();
	}
	setConditionFalse() {
		this.condition = false;
		if (this.rejectCondition) {
			this.rejectCondition(/* @__PURE__ */ new Error("Condition was set to false"));
			this.clearPromises();
		}
	}
	setConditionTrue() {
		this.condition = true;
		if (this.resolveCondition) {
			this.resolveCondition();
			this.clearPromises();
		}
	}
	waitForCondition() {
		return new Promise((resolve, reject) => {
			if (this.condition) resolve();
			else {
				this.resolveCondition = resolve;
				this.rejectCondition = reject;
			}
		});
	}
	clearPromises() {
		this.resolveCondition = null;
		this.rejectCondition = null;
	}
};
//#endregion
//#region src/utils/to.ts
/**
* @param { Readonly<Promise> } promise
* @param {object=} errorExt - Additional Information you can pass to the err object
* @return { Promise }
*/
async function to(promise, errorExt) {
	try {
		return [null, await promise];
	} catch (error) {
		if (errorExt) return [Object.assign({}, error, errorExt), void 0];
		return [error, void 0];
	}
}
//#endregion
//#region src/utils/tree.ts
/**
* @zh_CN 遍历树形结构，并返回所有节点中指定的值。
* @param tree 树形结构数组
* @param getValue 获取节点值的函数
* @param options 作为子节点数组的可选属性名称。
* @returns 所有节点中指定的值的数组
*/
function traverseTreeValues(tree, getValue, options) {
	const result = [];
	const { childProps } = options || { childProps: "children" };
	const dfs = (treeNode) => {
		const value = getValue(treeNode);
		result.push(value);
		const children = treeNode?.[childProps];
		if (!children) return;
		if (children.length > 0) for (const child of children) dfs(child);
	};
	for (const treeNode of tree) dfs(treeNode);
	return result.filter(Boolean);
}
/**
* 根据条件过滤给定树结构的节点，并以原有顺序返回所有匹配节点的数组。
* @param tree 要过滤的树结构的根节点数组。
* @param filter 用于匹配每个节点的条件。
* @param options 作为子节点数组的可选属性名称。
* @returns 包含所有匹配节点的数组。
*/
function filterTree(tree, filter, options) {
	const { childProps } = options || { childProps: "children" };
	const _filterTree = (nodes) => {
		return nodes.filter((node) => {
			if (filter(node)) {
				if (node[childProps]) node[childProps] = _filterTree(node[childProps]);
				return true;
			}
			return false;
		});
	};
	return _filterTree(tree);
}
/**
* 根据条件重新映射给定树结构的节
* @param tree 要过滤的树结构的根节点数组。
* @param mapper 用于map每个节点的条件。
* @param options 作为子节点数组的可选属性名称。
*/
function mapTree(tree, mapper, options) {
	const { childProps } = options || { childProps: "children" };
	return tree.map((node) => {
		const mapperNode = mapper(node);
		if (mapperNode[childProps]) mapperNode[childProps] = mapTree(mapperNode[childProps], mapper, options);
		return mapperNode;
	});
}
/**
* 对树形结构数据进行递归排序
* @param treeData - 树形数据数组
* @param sortFunction - 排序函数，用于定义排序规则
* @param options - 配置选项，包括子节点属性名
* @returns 排序后的树形数据
*/
function sortTree(treeData, sortFunction, options) {
	const { childProps } = options || { childProps: "children" };
	return treeData.toSorted(sortFunction).map((item) => {
		const children = item[childProps];
		if (children && Array.isArray(children) && children.length > 0) return {
			...item,
			[childProps]: sortTree(children, sortFunction, options)
		};
		return item;
	});
}
//#endregion
//#region src/utils/unique.ts
/**
* 根据指定字段对对象数组进行去重
* @param arr 要去重的对象数组
* @param key 去重依据的字段名
* @returns 去重后的对象数组
*/
function uniqueByField(arr, key) {
	const seen = /* @__PURE__ */ new Map();
	return arr.filter((item) => {
		const value = item[key];
		return seen.has(value) ? false : (seen.set(value, item), true);
	});
}
//#endregion
//#region src/utils/update-css-variables.ts
/**
* 更新 CSS 变量的函数
* @param variables 要更新的 CSS 变量与其新值的映射
*/
function updateCSSVariables(variables, id = "__vben-styles__") {
	const styleElement = document.querySelector(`#${id}`) || document.createElement("style");
	styleElement.id = id;
	let cssText = ":root {";
	for (const key in variables) if (Object.prototype.hasOwnProperty.call(variables, key)) cssText += `${key}: ${variables[key]};`;
	cssText += "}";
	styleElement.textContent = cssText;
	if (!document.querySelector(`#${id}`)) setTimeout(() => {
		document.head.append(styleElement);
	});
}
//#endregion
//#region src/utils/util.ts
function bindMethods(instance) {
	const prototype = Object.getPrototypeOf(instance);
	Object.getOwnPropertyNames(prototype).forEach((propertyName) => {
		const descriptor = Object.getOwnPropertyDescriptor(prototype, propertyName);
		const propertyValue = instance[propertyName];
		if (typeof propertyValue === "function" && propertyName !== "constructor" && descriptor && !descriptor.get && !descriptor.set) instance[propertyName] = propertyValue.bind(instance);
	});
}
/**
* 获取嵌套对象的字段值
* @param obj - 要查找的对象
* @param path - 用于查找字段的路径，使用小数点分隔
* @returns 字段值，或者未找到时返回 undefined
*/
function getNestedValue(obj, path) {
	if (typeof path !== "string" || path.length === 0) throw new Error("Path must be a non-empty string");
	const keys = path.split(".");
	let current = obj;
	for (const key of keys) {
		if (current === null || current === void 0) return;
		current = current[key];
	}
	return current;
}
//#endregion
export { Stack, StateHandler, arraysEqual, bindMethods, capitalizeFirstLetter, cloneDeep, cn, createMerge, createStack, diff, downloadFileFromBase64, downloadFileFromBlob, downloadFileFromBlobPart, downloadFileFromImageUrl, downloadFileFromUrl, filterTree, formatDate, formatDateTime, get, getCurrentTimezone, getElementVisibleRect, getFirstNonNullOrUndefined, getNestedValue, getScrollbarWidth, getSystemTimezone, isBoolean, isDate, isDayjsObject, isEmpty, isEqual, isFunction, isHttpUrl, isMacOs, isNumber, isObject, isString, isUndefined, isWindow, isWindowsOs, kebabToCamelCase, loadScript, mapTree, merge, mergeWithArrayOverride, needsScrollbar, openRouteInNewWindow, openWindow, set, setCurrentTimezone, sortTree, startProgress, stopProgress, to, toCamelCase, toLowerCaseFirstLetter, traverseTreeValues, triggerDownload, triggerWindowResize, uniqueByField, updateCSSVariables, urlToBase64 };
