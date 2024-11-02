import { z } from "zod";
import { readFile } from "node:fs/promises";
import Path from "node:path";
import jsYaml from "js-yaml";
import { assertPath, getTriggerTemplatePath } from "./utils.ts";

const NAME_RE = /^[a-z0-9_-]+$/;

const queueSchema = z.object({
  name: z.string().regex(NAME_RE),
  subjects: z.array(z.string()).nonempty(),
  max_age_secs: z.number().optional(),
  max_num_messages: z.number().optional(),
  duplicate_window_secs: z.number().optional(),
});

const streamSchema = z.object({
  name: z.string().regex(NAME_RE),
  subjects: z.array(z.string()).nonempty(),
  max_age_secs: z.number().optional(),
  max_num_messages: z.number().optional(),
  duplicate_window_secs: z.number().optional(),
});

const triggerSchema = z.discriminatedUnion("type", [
  z.object({
    type: z.literal("http"),
    route: z.string(),
  }),
  z.object({
    type: z.literal("pubsub"),
    subjects: z.array(z.string()),
  }),
  z.object({
    type: z.literal("stream"),
    name: z.string(),
    batch_size: z.number().default(1).optional(),
  }),
  z.object({
    type: z.literal("queue"),
    name: z.string(),
    batch_size: z.number().default(1).optional(),
  }),
]);

const functionSchema = z.object({
  name: z.string().regex(NAME_RE),
  runtime: z.string(),
  idle_timeout_secs: z.number().default(300).optional(),
  trigger: triggerSchema,
});

export const schema = z.object({
  name: z.string().regex(NAME_RE),
  http_port: z.number().default(8080).optional(),
  functions: z.array(functionSchema),
  queues: z.array(queueSchema).optional(),
  streams: z.array(streamSchema).optional(),
});

export type ConfigFileTrigger = z.infer<typeof triggerSchema>;

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
    await assertPath(Path.resolve(configPath, `functions/${f.name}`));
    await assertPath(getTriggerTemplatePath(f.runtime, f.trigger.type));
  }

  return {
    _raw: configJson as ConfigFile,
    ...config,
    queues: config.queues?.map((q) => ({
      ...q,
      get natsStreamName() {
        return `coupe_stack_${config.name}_queue_${q.name}`;
      },
    })),
    streams: config.streams?.map((s) => ({
      ...s,
      get natsStreamName() {
        return `coupe_stack_${config.name}_stream_${s.name}`;
      },
    })),
    functions: config.functions.map((f) => ({
      ...f,
      get containerName() {
        return `coupe_function_${config.name}_${f.name}`;
      },
      get natsStreamName() {
        if (f.trigger.type === "stream") {
          return `coupe_stack_${config.name}_stream_${f.trigger.name}`;
        } else if (f.trigger.type === "queue") {
          return `coupe_stack_${config.name}_queue_${f.trigger.name}`;
        } else {
          return null;
        }
      },
      get asyncResourceConfig() {
        return (
          f.trigger.type === "stream" ? config.streams : config.queues
        )?.find((s) => "name" in f.trigger && s.name === f.trigger.name);
      },
    })),
    get hasConsumerFunctions() {
      return (
        config.functions.find(
          (f) =>
            "name" in f.trigger &&
            (f.trigger.type === "queue" || f.trigger.type === "stream")
        ) !== undefined
      );
    },
    streamConfigByName(name: string) {
      return config.streams?.find((s) => s.name === name);
    },
    queueConfigByName(name: string) {
      return config.queues?.find((q) => q.name === name);
    },
  };
};

export type Config = Awaited<ReturnType<typeof loadConfig>>;

export type ConfigFunction = Config["functions"][number];

export interface CommandContext {
  config: Config;
  sourceDir: string;
}

export const getCommandContext = async (
  sourceDir = Path.resolve(Deno.cwd())
) => ({ sourceDir, config: await loadConfig(sourceDir) });
