import type { CommandContext } from "../config.ts";
import { STACK_DEPLOYMENT_DIR } from "../constants.ts";
import { $ } from "../utils.ts";

export const teardown = async (ctx: CommandContext) => {
  const deploymentDir = `${STACK_DEPLOYMENT_DIR}/${ctx.config.name}`;
  await $`docker-compose -f ${deploymentDir}/docker-compose.yaml down -v --remove-orphans --rmi all`;
  await Deno.remove(deploymentDir, { recursive: true });
};
