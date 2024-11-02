import { getCommandContext } from "./config.ts";
import * as commands from "./commands/index.ts";

const [command, ...params] = Deno.args;

if (!command) {
  console.error("No command provided");
  Deno.exit(1);
}

switch (command) {
  case "deploy":
    await commands.deploy(await getCommandContext());
    break;
  case "add":
    await commands.add(await getCommandContext(), params);
    break;
  case "init":
    await commands.init(params);
    break;
  case "scaffold":
    await commands.scaffold(await getCommandContext());
    break;
  default:
    console.error(`Unknown command: ${command}`);
    break;
}
