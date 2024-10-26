import { $ } from "execa";
import { loadConfig } from "./config.ts";
import * as commands from "./commands/index.ts";

const [command, ...params] = Deno.args;

if (!command) {
  console.error("No command provided");
  Deno.exit(1);
}

const sourceDir = (await $`pwd`).stdout.trim();
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
