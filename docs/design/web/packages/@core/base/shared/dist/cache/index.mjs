//#region src/cache/indexeddb-driver.ts
/**
* IndexedDB 驱动
* 采用懒初始化模式，首次操作时自动打开数据库
*/
var IndexedDBDriver = class {
	dbName;
	dbPromise = null;
	dbVersion;
	storeName;
	constructor({ dbName = "vben-storage", dbVersion = 1, storeName = "kv-store" } = {}) {
		this.dbName = dbName;
		this.dbVersion = dbVersion;
		this.storeName = storeName;
	}
	async clear() {
		const db = await this.getDB();
		return new Promise((resolve, reject) => {
			const tx = db.transaction(this.storeName, "readwrite");
			tx.objectStore(this.storeName).clear();
			tx.addEventListener("complete", () => resolve());
			tx.addEventListener("error", () => reject(tx.error));
			tx.addEventListener("abort", () => reject(tx.error ?? /* @__PURE__ */ new Error("Transaction aborted")));
		});
	}
	async getItem(key) {
		const db = await this.getDB();
		return new Promise((resolve, reject) => {
			const request = db.transaction(this.storeName, "readonly").objectStore(this.storeName).get(key);
			request.addEventListener("success", () => resolve(request.result ?? null));
			request.addEventListener("error", () => reject(request.error));
		});
	}
	async keys() {
		const db = await this.getDB();
		return new Promise((resolve, reject) => {
			const request = db.transaction(this.storeName, "readonly").objectStore(this.storeName).getAllKeys();
			request.addEventListener("success", () => resolve(request.result.map(String)));
			request.addEventListener("error", () => reject(request.error));
		});
	}
	async removeItem(key) {
		const db = await this.getDB();
		return new Promise((resolve, reject) => {
			const tx = db.transaction(this.storeName, "readwrite");
			tx.objectStore(this.storeName).delete(key);
			tx.addEventListener("complete", () => resolve());
			tx.addEventListener("error", () => reject(tx.error));
			tx.addEventListener("abort", () => reject(tx.error ?? /* @__PURE__ */ new Error("Transaction aborted")));
		});
	}
	async setItem(key, value) {
		const db = await this.getDB();
		return new Promise((resolve, reject) => {
			const tx = db.transaction(this.storeName, "readwrite");
			tx.objectStore(this.storeName).put(value, key);
			tx.addEventListener("complete", () => resolve());
			tx.addEventListener("error", () => reject(tx.error));
			tx.addEventListener("abort", () => reject(tx.error ?? /* @__PURE__ */ new Error("Transaction aborted")));
		});
	}
	/**
	* 懒初始化：首次调用时打开数据库，后续复用同一个 Promise
	*/
	getDB() {
		if (!this.dbPromise) this.dbPromise = this.openDB().catch((error) => {
			this.dbPromise = null;
			throw error;
		});
		return this.dbPromise;
	}
	openDB() {
		return new Promise((resolve, reject) => {
			const request = indexedDB.open(this.dbName, this.dbVersion);
			request.addEventListener("upgradeneeded", () => {
				const db = request.result;
				if (!db.objectStoreNames.contains(this.storeName)) db.createObjectStore(this.storeName);
			});
			request.addEventListener("success", () => resolve(request.result));
			request.addEventListener("error", () => reject(request.error));
		});
	}
};
//#endregion
//#region src/cache/local-storage-driver.ts
/**
* LocalStorage / SessionStorage 驱动
* 用 async 包装同步 API，保持接口统一
*/
var LocalStorageDriver = class {
	storage;
	constructor({ storageType = "localStorage" } = {}) {
		if (typeof window === "undefined") throw new Error("LocalStorageDriver is not available in non-browser environments. Use MemoryStorageDriver instead.");
		this.storage = storageType === "localStorage" ? window.localStorage : window.sessionStorage;
	}
	async clear() {
		this.storage.clear();
	}
	async getItem(key) {
		const raw = this.storage.getItem(key);
		if (raw === null) return null;
		try {
			return JSON.parse(raw);
		} catch {
			this.storage.removeItem(key);
			return null;
		}
	}
	async keys() {
		const result = [];
		for (let i = 0; i < this.storage.length; i++) {
			const key = this.storage.key(i);
			if (key !== null) result.push(key);
		}
		return result;
	}
	async removeItem(key) {
		this.storage.removeItem(key);
	}
	async setItem(key, value) {
		this.storage.setItem(key, JSON.stringify(value));
	}
};
//#endregion
//#region src/cache/memory-storage-driver.ts
/**
* 内存存储驱动
* 适用于测试环境和 SSR 场景，数据不持久化
*/
var MemoryStorageDriver = class {
	store = /* @__PURE__ */ new Map();
	async clear() {
		this.store.clear();
	}
	async getItem(key) {
		return this.store.get(key) ?? null;
	}
	async keys() {
		return [...this.store.keys()];
	}
	async removeItem(key) {
		this.store.delete(key);
	}
	async setItem(key, value) {
		this.store.set(key, value);
	}
};
//#endregion
//#region src/cache/storage-manager.ts
/**
* 存储管理器（策略模式）
* - prefix（命名空间隔离）在此层处理
* - TTL（过期机制）在此层处理
* - Driver 只负责纯粹的 KV 存取
*/
var StorageManager = class {
	driver;
	prefix;
	constructor({ driver, prefix = "" } = {}) {
		this.driver = driver || this.createDefaultDriver();
		this.prefix = prefix;
		if (!this.prefix && this.driver instanceof LocalStorageDriver) console.warn("[StorageManager] empty prefix combined with LocalStorageDriver — clear()/keys() will affect every localStorage entry.");
	}
	/**
	* 清除所有带前缀的存储项
	*/
	async clear() {
		const allKeys = await this.driver.keys();
		const fullPrefix = this.prefix ? `${this.prefix}-` : "";
		const prefixedKeys = allKeys.filter((key) => key.startsWith(fullPrefix));
		await Promise.all(prefixedKeys.map((key) => this.driver.removeItem(key)));
	}
	/**
	* 清除所有过期的存储项
	*/
	async clearExpiredItems() {
		const allKeys = await this.driver.keys();
		const fullPrefix = this.prefix ? `${this.prefix}-` : "";
		const prefixedKeys = allKeys.filter((key) => key.startsWith(fullPrefix));
		for (const fullKey of prefixedKeys) {
			const raw = await this.driver.getItem(fullKey);
			if (raw && raw.expiry && Date.now() > raw.expiry) await this.driver.removeItem(fullKey);
		}
	}
	/**
	* 获取存储项
	* @param key 键
	* @param defaultValue 当项不存在或已过期时返回的默认值
	* @returns 值，如果项已过期则返回默认值
	*/
	async getItem(key, defaultValue = null) {
		const fullKey = this.getFullKey(key);
		const raw = await this.driver.getItem(fullKey);
		if (!raw) return defaultValue;
		if (raw.expiry && Date.now() > raw.expiry) {
			await this.driver.removeItem(fullKey);
			return defaultValue;
		}
		return raw.value;
	}
	/**
	* 获取当前前缀下的所有存储键（已去除前缀部分）
	*/
	async keys() {
		const allKeys = await this.driver.keys();
		const fullPrefix = this.prefix ? `${this.prefix}-` : "";
		if (!fullPrefix) return allKeys;
		return allKeys.filter((key) => key.startsWith(fullPrefix)).map((key) => key.slice(fullPrefix.length));
	}
	/**
	* 移除存储项
	* @param key 键
	*/
	async removeItem(key) {
		const fullKey = this.getFullKey(key);
		await this.driver.removeItem(fullKey);
	}
	/**
	* 设置存储项
	* @param key 键
	* @param value 值
	* @param ttl 存活时间（毫秒）
	*/
	async setItem(key, value, ttl) {
		const fullKey = this.getFullKey(key);
		const item = {
			expiry: ttl ? Date.now() + ttl : void 0,
			value
		};
		await this.driver.setItem(fullKey, item);
	}
	/**
	* 根据运行环境创建默认驱动：
	* - 浏览器环境（window.localStorage 可用）→ LocalStorageDriver
	* - SSR / Node 环境 → MemoryStorageDriver
	*/
	createDefaultDriver() {
		try {
			if (typeof window !== "undefined" && window.localStorage) return new LocalStorageDriver();
		} catch (error) {
			console.warn("localStorage is not accessible, falling back to MemoryStorageDriver:", error);
		}
		return new MemoryStorageDriver();
	}
	/**
	* 获取完整的存储键（带前缀）
	* @param key 原始键
	* @returns 带前缀的完整键
	*/
	getFullKey(key) {
		return this.prefix ? `${this.prefix}-${key}` : key;
	}
};
//#endregion
export { IndexedDBDriver, LocalStorageDriver, MemoryStorageDriver, StorageManager };
