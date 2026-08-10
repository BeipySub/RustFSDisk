import { ClassValue } from "clsx";
import dayjs from "dayjs";
import { isFunction, isObject, isString } from "@vue/shared";
import { createDefu as createMerge, defu as merge } from "defu";
import { get, isEqual, set } from "es-toolkit/compat";
import cloneDeep from "lodash.clonedeep";

//#region src/utils/cn.d.ts
declare function cn(...inputs: ClassValue[]): string;
//#endregion
//#region src/utils/date.d.ts
type FormatDate = Date | dayjs.Dayjs | number | string;
type Format = 'HH' | 'HH:mm' | 'HH:mm:ss' | 'YYYY' | 'YYYY-MM' | 'YYYY-MM-DD' | 'YYYY-MM-DD HH' | 'YYYY-MM-DD HH:mm' | 'YYYY-MM-DD HH:mm:ss' | (string & {});
declare function formatDate(time?: FormatDate, format?: Format): string;
declare function formatDateTime(time?: FormatDate): string;
declare function isDate(value: any): value is Date;
declare function isDayjsObject(value: any): value is dayjs.Dayjs;
declare const getSystemTimezone: () => string;
declare const setCurrentTimezone: (timezone?: string) => void;
declare const getCurrentTimezone: () => string;
//#endregion
//#region src/utils/diff.d.ts
declare function arraysEqual<T>(a: T[], b: T[]): boolean;
type DiffResult<T> = Partial<{ [K in keyof T]: T[K] extends object ? DiffResult<T[K]> : T[K] }>;
declare function diff<T extends Record<string, any>>(obj1: T, obj2: T): DiffResult<T>;
//#endregion
//#region src/utils/dom.d.ts
interface VisibleDomRect {
  bottom: number;
  height: number;
  left: number;
  right: number;
  top: number;
  width: number;
}
declare function getElementVisibleRect(element?: HTMLElement | null | undefined): VisibleDomRect;
declare function getScrollbarWidth(): number;
declare function needsScrollbar(): boolean;
declare function triggerWindowResize(): void;
//#endregion
//#region src/utils/download.d.ts
interface DownloadOptions<T = string> {
  fileName?: string;
  source: T;
  target?: string;
}
declare function downloadFileFromUrl({
  fileName,
  source,
  target
}: DownloadOptions): Promise<void>;
declare function downloadFileFromBase64({
  fileName,
  source
}: DownloadOptions): void;
declare function downloadFileFromImageUrl({
  fileName,
  source
}: DownloadOptions): Promise<void>;
declare function downloadFileFromBlob({
  fileName,
  source
}: DownloadOptions<Blob>): void;
declare function downloadFileFromBlobPart({
  fileName,
  source
}: DownloadOptions<BlobPart>): void;
declare function urlToBase64(url: string, mineType?: string): Promise<string>;
declare function triggerDownload(href: string, fileName: string | undefined, revokeDelay?: number): void;
//#endregion
//#region src/utils/inference.d.ts
declare function isUndefined(value?: unknown): value is undefined;
declare function isBoolean(value: unknown): value is boolean;
declare function isEmpty<T = unknown>(value?: T): value is T;
declare function isHttpUrl(url?: string): boolean;
declare function isWindow(value: any): value is Window;
declare function isMacOs(): boolean;
declare function isWindowsOs(): boolean;
declare function isNumber(value: any): value is number;
declare function getFirstNonNullOrUndefined<T>(...values: (null | T | undefined)[]): T | undefined;
//#endregion
//#region src/utils/letter.d.ts
declare function capitalizeFirstLetter(string: string): string;
declare function toLowerCaseFirstLetter(str: string): string;
declare function toCamelCase(key: string, parentKey: string): string;
declare function kebabToCamelCase(str: string): string;
//#endregion
//#region src/utils/merge.d.ts
declare const mergeWithArrayOverride: import("defu").DefuFn;
//#endregion
//#region src/utils/nprogress.d.ts
declare function startProgress(): Promise<void>;
declare function stopProgress(): Promise<void>;
//#endregion
//#region src/utils/resources.d.ts
declare function loadScript(src: string): Promise<void>;
//#endregion
//#region src/utils/stack.d.ts
declare class Stack<T> {
  get size(): number;
  private readonly dedup;
  private items;
  private readonly maxSize?;
  constructor(dedup?: boolean, maxSize?: number);
  clear(): void;
  peek(): T | undefined;
  pop(): T | undefined;
  push(...items: T[]): void;
  remove(...itemList: T[]): void;
  retain(itemList: T[]): void;
  toArray(): T[];
}
declare const createStack: <T>(dedup?: boolean, maxSize?: number) => Stack<T>;
//#endregion
//#region src/utils/state-handler.d.ts
declare class StateHandler {
  private condition;
  private rejectCondition;
  private resolveCondition;
  isConditionTrue(): boolean;
  reset(): void;
  setConditionFalse(): void;
  setConditionTrue(): void;
  waitForCondition(): Promise<void>;
  private clearPromises;
}
//#endregion
//#region src/utils/to.d.ts
declare function to<T, U = Error>(promise: Readonly<Promise<T>>, errorExt?: object): Promise<[null, T] | [U, undefined]>;
//#endregion
//#region src/utils/tree.d.ts
interface TreeConfigOptions {
  childProps: string;
}
declare function traverseTreeValues<T, V>(tree: T[], getValue: (node: T) => V, options?: TreeConfigOptions): V[];
declare function filterTree<T extends Record<string, any>>(tree: T[], filter: (node: T) => boolean, options?: TreeConfigOptions): T[];
declare function mapTree<T, V extends Record<string, any>>(tree: T[], mapper: (node: T) => V, options?: TreeConfigOptions): V[];
declare function sortTree<T extends Record<string, any>>(treeData: T[], sortFunction: (a: T, b: T) => number, options?: TreeConfigOptions): T[];
//#endregion
//#region src/utils/unique.d.ts
declare function uniqueByField<T>(arr: T[], key: keyof T): T[];
//#endregion
//#region src/utils/update-css-variables.d.ts
declare function updateCSSVariables(variables: {
  [key: string]: string;
}, id?: string): void;
//#endregion
//#region src/utils/util.d.ts
declare function bindMethods<T extends object>(instance: T): void;
declare function getNestedValue<T>(obj: T, path: string): any;
//#endregion
//#region src/utils/window.d.ts
interface OpenWindowOptions {
  noopener?: boolean;
  noreferrer?: boolean;
  target?: '_blank' | '_parent' | '_self' | '_top' | string;
}
declare function openWindow(url: string, options?: OpenWindowOptions): void;
declare function openRouteInNewWindow(path: string): void;
//#endregion
export { Stack, StateHandler, VisibleDomRect, arraysEqual, bindMethods, capitalizeFirstLetter, cloneDeep, cn, createMerge, createStack, diff, downloadFileFromBase64, downloadFileFromBlob, downloadFileFromBlobPart, downloadFileFromImageUrl, downloadFileFromUrl, filterTree, formatDate, formatDateTime, get, getCurrentTimezone, getElementVisibleRect, getFirstNonNullOrUndefined, getNestedValue, getScrollbarWidth, getSystemTimezone, isBoolean, isDate, isDayjsObject, isEmpty, isEqual, isFunction, isHttpUrl, isMacOs, isNumber, isObject, isString, isUndefined, isWindow, isWindowsOs, kebabToCamelCase, loadScript, mapTree, merge, mergeWithArrayOverride, needsScrollbar, openRouteInNewWindow, openWindow, set, setCurrentTimezone, sortTree, startProgress, stopProgress, to, toCamelCase, toLowerCaseFirstLetter, traverseTreeValues, triggerDownload, triggerWindowResize, uniqueByField, updateCSSVariables, urlToBase64 };