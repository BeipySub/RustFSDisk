import { consola, execaCommand, getPackages } from "@vben/node-utils";
import { cac } from "cac";
import { cancel, isCancel, select } from "@clack/prompts";
//#region src/run.ts
async function run(options) {
	const { command } = options;
	if (!command) {
		console.error("Please enter the command to run");
		process.exit(1);
	}
	const { packages } = await getPackages();
	const selectPkgs = packages.filter((pkg) => {
		return (pkg?.packageJson)?.scripts?.[command];
	});
	let selectPkg;
	if (selectPkgs.length > 1) {
		selectPkg = await select({
			message: `Select the app you need to run [${command}]:`,
			options: selectPkgs.map((item) => ({
				label: item?.packageJson.name,
				value: item?.packageJson.name
			}))
		});
		if (isCancel(selectPkg) || !selectPkg) {
			cancel("👋 Has cancelled");
			process.exit(0);
		}
	} else selectPkg = selectPkgs[0]?.packageJson?.name ?? "";
	if (!selectPkg) {
		console.error("No app found");
		process.exit(1);
	}
	execaCommand(`pnpm --filter=${selectPkg} run ${command}`, { stdio: "inherit" });
}
/**
* 过滤app包
* @param root
* @param packages
*/
//#endregion
//#region src/index.ts
try {
	const turboRun = cac("turbo-run");
	turboRun.command("[script]").usage(`Run turbo interactively.`).action(async (command) => {
		run({ command });
	});
	turboRun.usage("turbo-run");
	turboRun.help();
	turboRun.parse();
} catch (error) {
	consola.error(error);
	process.exit(1);
}
//#endregion
export {};
