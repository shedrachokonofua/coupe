import { $ } from "execa";
import fs from "node:fs/promises";
import fse from "fs-extra";
import jsonToYaml from "json-to-pretty-yaml";
import { RetentionPolicy, DeliverPolicy } from "nats";
import type { Config } from "../config.ts";
import {
  cleanFolder,
  dropStartEndSlash,
  getFunctionTemplatePath,
  getRandomNumberInRange,
  secsToNanoSecs,
} from "../utils.ts";
import { STACK_DEPLOYMENT_DIR } from "../constants.ts";
import { AckPolicy } from "nats";
import { NatsClient } from "../nats.ts";

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
    const fnBuildDir = `${deploymentDir}/functions/${f.name}`;
    const handlerBuildDir = `${fnBuildDir}/handler`;
    await cleanFolder(fnBuildDir);

    // Copy the template files to the build directory, and remove the template handler
    await $`cp -r ${templateDir}/. ${fnBuildDir}`;
    await cleanFolder(handlerBuildDir);

    // Copy the source handler to the build directory
    await $`cp -r ${handlerSourceDir}/. ${fnBuildDir}/handler`;

    // If .env file exists in the handler build directory, move it into the parent directory
    try {
      await $`mv ${handlerBuildDir}/.env ${fnBuildDir}`;
      containersWithEnvFiles.add(f.containerName);
    } catch (_e) {
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
  } catch (_e) {
    // If inspection fails, the image doesn't exist, so we build it
    console.log("caddy-with-sablier image not found, building...");
    await $`docker build https://github.com/acouvreur/sablier.git#v1.4.0-beta.3:plugins/caddy \
        --build-arg=CADDY_VERSION=2.6.4 \
        -t caddy:2.6.4-with-sablier`;
    console.log("caddy-with-sablier image built successfully.");
  }

  const shouldUseNats = ctx.config.functions.some(
    (f) =>
      f.trigger.type === "pubsub" ||
      f.trigger.type === "stream" ||
      f.trigger.type === "queue"
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
          "./platform/caddy/Caddyfile:/etc/caddy/Caddyfile",
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
    dockerComposeJson.services.nats = {
      container_name: `coupe_stack_${ctx.config.name}_nats`,
      image: "nats:latest",
      command: ["--js", "--sd=/data"],
      restart: "unless-stopped",
      profiles: ["platform"],
      ports: [`${natsHostPort}:4222`],
      volumes: ["nats_data:/data"],
    };
    dockerComposeJson.volumes.nats_data = null;
  }

  if (ctx.config.hasConsumerFunctions) {
    const wakerSubscriptionConfig: Record<string, string[]> = {};
    for (const f of ctx.config.functions) {
      if (
        (f.trigger.type === "queue" || f.trigger.type === "stream") &&
        f.asyncResourceConfig
      ) {
        for (const subject of f.asyncResourceConfig.subjects) {
          wakerSubscriptionConfig[subject] =
            wakerSubscriptionConfig[subject] || [];
          wakerSubscriptionConfig[subject].push(f.containerName);
        }
      }

      dockerComposeJson.services.consumer_function_waker = {
        container_name: `coupe_stack_${ctx.config.name}_consumer_function_waker`,
        image: "coupe/consumer-function-waker",
        environment: {
          NATS_URL: "nats://nats:4222",
          SUBSCRIPTION_CONFIG: JSON.stringify(wakerSubscriptionConfig),
        },
        restart: "unless-stopped",
        depends_on: ["nats"],
        profiles: ["platform"],
      };
    }
  }

  for (const f of ctx.config.functions) {
    const isHttpTrigger = f.trigger.type === "http";
    dockerComposeJson.services[f.containerName] = {
      container_name: f.containerName,
      build: `./functions/${f.name}`,
      labels: [
        `sablier.enable=${f.trigger.type !== "pubsub"}`,
        `sablier.group=${f.containerName}`,
      ],
      profiles: ["function", f.trigger.type, isHttpTrigger ? "sync" : "async"],
      environment: {
        FUNCTION_NAME: f.name,
        CONTAINER_NAME: f.containerName,
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
                route ${f.trigger.route} {
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
            case "queue":
            case "stream":
              return `
                route /__coupe/${f.containerName}/wake {
                  sablier {
                    group ${f.containerName}
                    session_duration ${f.idle_timeout_secs}s

                    blocking {
                      timeout 30s
                    }
                  }
                  
                  respond 200
                }
              `;
            default:
              return "";
          }
        })
        .join("\n")}
    }
  `;
  const caddyDeploymentDir = `${deploymentDir}/platform/caddy`;
  await fse.outputFile(`${caddyDeploymentDir}/Caddyfile`, caddyFileContent);
  // Format the Caddyfile
  await $`docker run --rm -v ${caddyDeploymentDir}:/app caddy:2.6.4-with-sablier caddy fmt --overwrite /app/Caddyfile`;

  // Rebuild platform docker containers
  await $`docker-compose -f ${deploymentDir}/docker-compose.yaml --profile platform up --build --force-recreate -d`;

  // Setup nats streams
  if (shouldUseNats && ctx.config.hasConsumerFunctions) {
    // Expose port from nats container to the host
    const nc = await NatsClient.connect(natsHostPort);

    for (const queue of ctx.config.queues || []) {
      await nc.getOrCreateStream({
        name: queue.natsStreamName,
        subjects: queue.subjects,
        retention: RetentionPolicy.Workqueue,
        max_msgs: queue.max_num_messages,
        max_age: queue.max_age_secs
          ? secsToNanoSecs(queue.max_age_secs)
          : undefined,
      });
    }

    for (const stream of ctx.config.streams || []) {
      await nc.getOrCreateStream({
        name: stream.natsStreamName,
        subjects: stream.subjects,
        retention: RetentionPolicy.Limits,
        max_msgs: stream.max_num_messages,
        max_age: stream.max_age_secs
          ? secsToNanoSecs(stream.max_age_secs)
          : undefined,
      });
    }

    for (const f of ctx.config.functions) {
      if (
        (f.trigger.type === "stream" || f.trigger.type === "queue") &&
        f.natsStreamName
      ) {
        await nc.getOrCreateConsumer(f.natsStreamName, {
          durable_name: f.containerName,
          max_batch: f.trigger.batch_size,
          ack_policy: AckPolicy.Explicit,
          deliver_policy: DeliverPolicy.All,
        });
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
