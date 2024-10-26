import { $ } from "execa";
import fs from "node:fs/promises";
import jsonToYaml from "json-to-pretty-yaml";
import nats, { RetentionPolicy } from "nats";
import type { Config } from "./config.ts";
import {
  assertPath,
  cleanFolder,
  doesPathExist,
  dropStartEndSlash,
  getFunctionTemplatePath,
  getHandlerTemplatePath,
  getRandomNumberInRange,
  secsToNanoSecs,
} from "./utils.ts";
import { STACK_DEPLOYMENT_DIR } from "./constants.ts";

interface CommandContext {
  config: Config;
  sourceDir: string;
}

export const deploy = async (ctx: CommandContext) => {
  const deploymentDir = `${STACK_DEPLOYMENT_DIR}/${ctx.config.name}`;
  const containersWithEnvFiles = new Set();

  // Prepare function build directories
  for (const f of ctx.config.functions) {
    const templateDir = getFunctionTemplatePath(f.runtime, f.trigger.type);
    const handlerSourceDir = `${ctx.sourceDir}/${f.path}`;
    const fnBuildDir = `${deploymentDir}/${f.name}`;
    const handlerBuildDir = `${fnBuildDir}/handler`;
    await cleanFolder(fnBuildDir);

    // Copy the template files to the build directory, and remove the template handler
    await $`cp -r ${templateDir}/* ${fnBuildDir}`;
    await cleanFolder(handlerBuildDir);

    // Copy the source handler to the build directory
    await $`cp -r ${handlerSourceDir}/* ${fnBuildDir}/handler`;

    // If .env file exists in the handler build directory, move it into the parent directory
    try {
      await $`mv ${handlerBuildDir}/.env ${fnBuildDir}`;
      containersWithEnvFiles.add(f.containerName);
    } catch (e) {
      // Ignore if the file does not exist
      $`echo "No .env file found for function ${f.name}, skipping..."`;
    }
  }

  // Build docker-compose
  // Build caddy-with-sablier image if it does not exist
  try {
    // Try to inspect the image
    await $`docker image inspect caddy:2.6.4-with-sablier`;
    console.log("caddy-with-sablier image already exists, skipping build.");
  } catch (error) {
    // If inspection fails, the image doesn't exist, so we build it
    console.log("caddy-with-sablier image not found, building...");
    await $`docker build https://github.com/acouvreur/sablier.git#v1.4.0-beta.3:plugins/caddy \
        --build-arg=CADDY_VERSION=2.6.4 \
        -t caddy:2.6.4-with-sablier`;
    console.log("caddy-with-sablier image built successfully.");
  }

  const shouldUseNats = ctx.config.functions.some(
    (f) => f.trigger.type === "pubsub"
  );

  const dockerComposeJson = {
    name: `coupe_stack_${ctx.config.name}`,
    services: {
      sablier: {
        container_name: `coupe_stack_${ctx.config.name}_sablier`,
        image: "coupe/sablier", // Had to make it cold-start faster(https://github.com/acouvreur/sablier/issues/282), cloned it and made some changes -> pc/projects/sablier
        command: ["start", "--provider.name=docker"],
        volumes: ["/var/run/docker.sock:/var/run/docker.sock"],
        profiles: ["platform"],
      },
      caddy: {
        container_name: `coupe_stack_${ctx.config.name}_caddy`,
        image: "caddy:2.6.4-with-sablier",
        ports: [`${ctx.config.http_port}:80`],
        restart: "unless-stopped",
        volumes: [
          "./Caddyfile:/etc/caddy/Caddyfile",
          "caddy_data:/data",
          "caddy_config:/config",
        ],
        depends_on: ["sablier"],
        profiles: ["platform"],
      },
    },
    volumes: {
      caddy_data: null,
      caddy_config: null,
    },
  } as any;

  const natsHostPort = getRandomNumberInRange(56000, 57000);
  if (shouldUseNats) {
    Object.assign(dockerComposeJson.services, {
      nats: {
        container_name: `coupe_stack_${ctx.config.name}_nats`,
        image: "nats:latest",
        restart: "unless-stopped",
        profiles: ["platform"],
        ports: [`${natsHostPort}:4222`],
      },
      natscli: {
        container_name: `coupe_stack_${ctx.config.name}_natscli`,
        image: "bitnami/natscli:latest",
        depends_on: ["nats"],
        profiles: ["platform"],
      },
    });
  }

  for (const f of ctx.config.functions) {
    dockerComposeJson.services[f.containerName] = {
      container_name: f.containerName,
      build: `./${f.name}`,
      labels: [
        `sablier.enable=${f.trigger.type === "http"}`,
        `sablier.group=${f.containerName}`,
      ],
      profiles: ["function", f.trigger.type],
      environment: {
        FUNCTION_NAME: f.name,
        FUNCTION_CONTAINER_NAME: f.containerName,
        IDLE_TIMEOUT_SECS: f.idle_timeout_secs,
      },
    };

    if (shouldUseNats) {
      Object.assign(dockerComposeJson.services[f.containerName].environment, {
        NATS_URL: "nats://nats:4222",
      });
    }

    if (f.trigger.type === "pubsub") {
      Object.assign(dockerComposeJson.services[f.containerName].environment, {
        SUBJECTS: f.trigger.subjects.join(","),
      });
    }

    if (f.trigger.type === "stream") {
      const streamConfig = ctx.config.streams?.find(
        (s) => "name" in f.trigger && s.name === f.trigger.name
      );
      if (!streamConfig) {
        throw new Error(`Stream ${f.trigger.name} not found in config.`);
      }
      Object.assign(dockerComposeJson.services[f.containerName].environment, {
        STREAM_NAME: f.trigger.name,
        NATS_STREAM_NAME: streamConfig.natsStreamName,
        BATCH_SIZE: f.trigger.batch_size,
      });
    }

    if (f.trigger.type === "queue") {
      const queueConfig = ctx.config.queues?.find(
        (q) => "name" in f.trigger && q.name === f.trigger.name
      );
      if (!queueConfig) {
        throw new Error(`Queue ${f.trigger.name} not found in config.`);
      }
      Object.assign(dockerComposeJson.services[f.containerName].environment, {
        QUEUE_NAME: f.trigger.name,
        NATS_STREAM_NAME: queueConfig.natsStreamName,
        BATCH_SIZE: f.trigger.batch_size,
      });
    }

    if (containersWithEnvFiles.has(f.containerName)) {
      dockerComposeJson.services[f.containerName].env_file = [".env"];
    }
  }

  const dockerComposeYaml = jsonToYaml.stringify(dockerComposeJson);
  await fs.writeFile(`${deploymentDir}/docker-compose.yaml`, dockerComposeYaml);

  // Build caddy file
  const caddyFileContent = `
    {
      debug
    }

    :80 {
      ${ctx.config.functions
        .map((f) => {
          switch (f.trigger.type) {
            case "http":
              return `
                route /${dropStartEndSlash(f.trigger.route)} {
                  sablier {
                    group ${f.containerName}
                    session_duration ${f.idle_timeout_secs}s

                    blocking {
                      timeout 30s
                    }
                  }
                  reverse_proxy ${f.containerName}
                }
                `;
            default:
              return ``;
          }
        })
        .join("\n")}
    }
    `;
  await fs.writeFile(`${deploymentDir}/Caddyfile`, caddyFileContent);
  // Format the Caddyfile
  await $`docker run --rm -v ${deploymentDir}:/app caddy:2.6.4-with-sablier caddy fmt --overwrite /app/Caddyfile`;

  // Rebuild platform docker containers
  await $`docker-compose -f ${deploymentDir}/docker-compose.yaml --profile platform up --build -d`;

  const shouldSetupNatsStreams =
    (ctx.config.streams || []).length > 0 ||
    (ctx.config.queues || []).length > 0;
  // Setup nats streams
  if (shouldUseNats && shouldSetupNatsStreams) {
    // Expose port from nats container to the host
    const nc = await nats.connect({
      servers: [`nats://localhost:${natsHostPort}`],
    });
    const jsm = await nc.jetstreamManager();

    for (const queue of ctx.config.queues || []) {
      try {
        await jsm.streams.add({
          name: queue.natsStreamName,
          subjects: queue.subjects,
          retention: RetentionPolicy.Workqueue,
          max_msgs: queue.max_num_messages,
          max_age: queue.max_age_secs
            ? secsToNanoSecs(queue.max_age_secs)
            : undefined,
        });
      } catch (error) {
        console.error(
          `Error creating queue ${queue.name}: ${JSON.stringify(
            error
          )}, skipping...`
        );
      }
    }

    for (const stream of ctx.config.streams || []) {
      try {
        await jsm.streams.add({
          name: stream.natsStreamName,
          subjects: stream.subjects,
          retention: RetentionPolicy.Limits,
          max_msgs: stream.max_num_messages,
          max_age: stream.max_age_secs
            ? secsToNanoSecs(stream.max_age_secs)
            : undefined,
        });
      } catch (error) {
        console.error(
          `Error creating stream ${stream.name}: ${JSON.stringify(
            error
          )}, skipping...`
        );
      }
    }

    for (const f of ctx.config.functions) {
      if (f.trigger.type === "stream" || f.trigger.type === "queue") {
        try {
          await jsm.consumers.add(f.trigger.name, {
            durable_name: f.containerName,
            max_batch: f.trigger.batch_size,
          });
        } catch (error) {
          console.error(
            `Error creating consumer for ${f.trigger.type} ${
              f.trigger.name
            }: ${JSON.stringify(error)}, skipping...`
          );
        }
      }
    }

    await nc.close();
  }

  // Build functions docker images
  await $`docker-compose -f ${deploymentDir}/docker-compose.yaml --profile function create --build --force-recreate`;

  // Start pubsub trigger functions
  await $`docker-compose -f ${deploymentDir}/docker-compose.yaml --profile pubsub up -d`;

  await $`echo "Deployment complete!"`;
};

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
