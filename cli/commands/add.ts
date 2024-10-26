import { $ } from "execa";
import fs from "node:fs/promises";
import jsonToYaml from "json-to-pretty-yaml";
import type { Config } from "../config.ts";
import { assertPath, doesPathExist, getHandlerTemplatePath } from "../utils.ts";

interface CommandContext {
  config: Config;
  sourceDir: string;
}

export const add = async (ctx: CommandContext, params: string[]) => {
  if (params.length !== 3) {
    throw new Error("Invalid number of arguments");
  }
  const [name, runtime, trigger] = params;
  const templateDir = getHandlerTemplatePath(runtime, trigger);
  assertPath(templateDir);

  const newFnPath = `${ctx.sourceDir}/${name}`;
  if (await doesPathExist(newFnPath)) {
    throw new Error(`Function ${name} already exists.`);
  }

  await $`cp -r ${templateDir} ${newFnPath}`;

  const configJson = ctx.config._raw as any;
  configJson.functions.push({
    name,
    path: `./${name}`,
    runtime,
    trigger: {
      type: trigger,
      ...(trigger === "http" ? { route: `/${name}` } : { name: "" }),
    },
  });
  const configYaml = jsonToYaml.stringify(configJson);
  await fs.writeFile(`${ctx.sourceDir}/coupe.yaml`, configYaml);
};
