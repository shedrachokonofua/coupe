import { z } from "zod";
import { readFile } from "fs/promises";
import Path from "path";
import jsYaml from "js-yaml";
import { assertPath, getTriggerTemplatePath } from "./utils";

const NAME_RE = /^[a-z0-9_-]+$/;

const functionSchema = z.object({
  name: z.string().regex(NAME_RE),
  path: z.string(),
  runtime: z.string(),
  idle_timeout_secs: z.number().optional().default(300),
  trigger: z.discriminatedUnion("type", [
    z.object({
      type: z.literal("http"),
      route: z.string(),
    }),
    z.object({
      type: z.literal("pubsub"),
      subjects: z.array(z.string()).nonempty(),
    }),
  ]),
});

export const schema = z.object({
  name: z.string().regex(NAME_RE),
  http_port: z.number(),
  functions: z.array(functionSchema).nonempty(),
});

export type ConfigFileFunction = z.infer<typeof functionSchema>;

export type ConfigFile = z.infer<typeof schema>;

export const loadConfig = async (configPath: string) => {
  const configFileContent = await readFile(
    Path.resolve(configPath, "coupe.yaml"),
    "utf-8"
  );
  const configJson = jsYaml.load(configFileContent);
  const config = schema.parse(configJson);

  for (const f of config.functions) {
    await assertPath(Path.resolve(configPath, f.path));
    await assertPath(getTriggerTemplatePath(f.runtime, f.trigger.type));
  }

  return {
    _raw: configJson,
    ...config,
    functions: config.functions.map((f) => ({
      ...f,
      get containerName() {
        return `coupe_function_${config.name}_${f.name}`;
      },
    })),
  };
};

export type Config = Awaited<ReturnType<typeof loadConfig>>;

export type ConfigFunction = Config["functions"][number];
