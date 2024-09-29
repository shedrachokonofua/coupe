import { $ } from "bun";
import { parseArgs } from "util";
import { loadConfig } from "./config";
import * as commands from "./commands";

const { positionals } = parseArgs({
  args: Bun.argv,
  allowPositionals: true,
});
const [command, ...params] = positionals.slice(2);

if (!command) {
  console.error("No command provided");
  process.exit(1);
}

const workspaceSourceDir = (await $`pwd`).text().trim();
const config = await loadConfig(workspaceSourceDir);
const context = { config, workspaceSourceDir };

switch (command) {
  case "deploy":
    await commands.deploy(context, params);
    break;
  case "stop":
    await commands.stop(context, params);
    break;
  default:
    console.error(`Unknown command: ${command}`);
    break;
}
