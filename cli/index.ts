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

const sourceDir = (await $`pwd`).text().trim();
const config = await loadConfig(sourceDir);
const context = { config, sourceDir };

switch (command) {
  case "deploy":
    await commands.deploy(context);
    break;
  case "add":
    await commands.add(context, params);
    break;
  default:
    console.error(`Unknown command: ${command}`);
    break;
}
