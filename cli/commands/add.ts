import { $ } from "../utils.ts";
import fs from "node:fs/promises";
import jsonToYaml from "json-to-pretty-yaml";
import type { CommandContext, ConfigFileTrigger } from "../config.ts";
import { assertPath, doesPathExist, getHandlerTemplatePath } from "../utils.ts";
import { scaffoldRuntimePackages } from "./scaffold.ts";

const getTriggerConfig = (trigger: string, name: string): ConfigFileTrigger => {
  switch (trigger) {
    case "http":
      return { type: trigger, route: `/${name}` };
    case "pubsub":
      return { type: trigger, subjects: [] };
    case "stream":
    case "queue":
      return { type: trigger, name: "" };
    default:
      throw new Error(`Unknown trigger type: ${trigger}`);
  }
};

export const add = async (ctx: CommandContext, params: string[]) => {
  if (params.length !== 3) {
    throw new Error("Invalid number of arguments");
  }
  const [name, runtime, trigger] = params;
  const templateDir = getHandlerTemplatePath(runtime, trigger);
  assertPath(templateDir);

  const newFnPath = `${ctx.sourceDir}/functions/${name}`;
  if (await doesPathExist(newFnPath)) {
    throw new Error(`Function ${name} already exists.`);
  }

  await $`cp -r ${templateDir} ${newFnPath}`;

  const configJson = ctx.config._raw;
  configJson.functions.push({
    name,
    runtime,
    trigger: getTriggerConfig(trigger, name),
  });
  const configYaml = jsonToYaml.stringify(configJson);
  await fs.writeFile(`${ctx.sourceDir}/coupe.yaml`, configYaml);

  await scaffoldRuntimePackages(ctx, runtime);
};
