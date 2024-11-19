import type { CommandContext } from "../config.ts";
import { $ } from "../utils.ts";

export const teardown = async (ctx: CommandContext) => {
  const deploymentDir = `${ctx.sourceDir}/build`;
  await $`docker-compose -f ${deploymentDir}/docker-compose.yaml down -v --remove-orphans --rmi all`;
  await Deno.remove(deploymentDir, { recursive: true });
};
