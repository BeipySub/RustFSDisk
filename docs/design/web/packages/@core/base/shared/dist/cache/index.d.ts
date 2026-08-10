//#region src/cache/types.d.ts
interface IStorageDriver {
  clear(): Promise<void>;
  getItem<T>(key: string): Promise<null | T>;
  keys(): Promise<string[]>;
  removeItem(key: string): Promise<void>;
  setItem(key: string, value: unknown): Promise<void>;
}
interface StorageItem<T> {
  expiry?: number;
  value: T;
}
interface StorageManagerOptions {
  driver?: IStorageDriver;
  prefix?: string;
}
//#endregion
//#region src/cache/indexeddb-driver.d.ts
interface IndexedDBDriverOptions {
  dbName?: string;
  dbVersion?: number;
  storeName?: string;
}
declare class IndexedDBDriver implements IStorageDriver {
  private dbName;
  private dbPromise;
  private dbVersion;
  private storeName;
  constructor({
    dbName,
    dbVersion,
    storeName
  }?: IndexedDBDriverOptions);
  clear(): Promise<void>;
  getItem<T>(key: string): Promise<null | T>;
  keys(): Promise<string[]>;
  removeItem(key: string): Promise<void>;
  setItem(key: string, value: unknown): Promise<void>;
  private getDB;
  private openDB;
}
//#endregion
//#region src/cache/local-storage-driver.d.ts
type StorageType = 'localStorage' | 'sessionStorage';
interface LocalStorageDriverOptions {
  storageType?: StorageType;
}
declare class LocalStorageDriver implements IStorageDriver {
  private storage;
  constructor({
    storageType
  }?: LocalStorageDriverOptions);
  clear(): Promise<void>;
  getItem<T>(key: string): Promise<null | T>;
  keys(): Promise<string[]>;
  removeItem(key: string): Promise<void>;
  setItem(key: string, value: unknown): Promise<void>;
}
//#endregion
//#region src/cache/memory-storage-driver.d.ts
declare class MemoryStorageDriver implements IStorageDriver {
  private store;
  clear(): Promise<void>;
  getItem<T>(key: string): Promise<null | T>;
  keys(): Promise<string[]>;
  removeItem(key: string): Promise<void>;
  setItem(key: string, value: unknown): Promise<void>;
}
//#endregion
//#region src/cache/storage-manager.d.ts
declare class StorageManager {
  private driver;
  private prefix;
  constructor({
    driver,
    prefix
  }?: StorageManagerOptions);
  clear(): Promise<void>;
  clearExpiredItems(): Promise<void>;
  getItem<T>(key: string, defaultValue?: null | T): Promise<null | T>;
  keys(): Promise<string[]>;
  removeItem(key: string): Promise<void>;
  setItem(key: string, value: unknown, ttl?: number): Promise<void>;
  private createDefaultDriver;
  private getFullKey;
}
//#endregion
export { type IStorageDriver, IndexedDBDriver, type IndexedDBDriverOptions, LocalStorageDriver, type LocalStorageDriverOptions, MemoryStorageDriver, type StorageItem, StorageManager, type StorageManagerOptions };