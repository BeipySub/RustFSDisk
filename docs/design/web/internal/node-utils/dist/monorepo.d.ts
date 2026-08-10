import type { Package } from '@manypkg/get-packages';
import * as manypkg from '@manypkg/get-packages';
declare function findMonorepoRoot(cwd?: string): string;
declare function getPackagesSync(): manypkg.Packages;
declare function getPackages(): Promise<manypkg.Packages>;
declare function getPackage(pkgName: string): Promise<Package | undefined>;
export { findMonorepoRoot, getPackage, getPackages, getPackagesSync };
