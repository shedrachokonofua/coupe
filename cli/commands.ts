import { $ } from "bun";
import fs from "fs/promises";
import jsonToYaml from "json-to-pretty-yaml";
import type { Config } from "./config";
import {
  assertPath,
  cleanFolder,
  doesPathExist,
  dropStartEndSlash,
  getFunctionTemplatePath,
  getHandlerTemplatePath,
} from "./utils";
import { STACK_DEPLOYMENT_DIR } from "./constants";

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

  if (shouldUseNats) {
    Object.assign(dockerComposeJson.services, {
      nats: {
        container_name: `coupe_stack_${ctx.config.name}_nats`,
        image: "nats:latest",
        restart: "unless-stopped",
        profiles: ["platform"],
      },
      nats_ui: {
        container_name: `coupe_stack_${ctx.config.name}_nats_ui`,
        image: "ghcr.io/nats-nui/nui:latest",
        volumes: ["nats_ui_db:/db"],
        profiles: ["platform"],
        ports: ["31311:31311"],
      },
    });
    dockerComposeJson.volumes.nats_ui_db = null;
  }

  for (const f of ctx.config.functions) {
    dockerComposeJson.services[f.containerName] = {
      container_name: f.containerName,
      build: `./${f.name}`,
      labels: [
        `sablier.enable=${f.trigger.type !== "pubsub"}`,
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
      ...(trigger === "http" ? { route: `/${name}` } : { subjects: [] }),
    },
  });
  const configYaml = jsonToYaml.stringify(configJson);
  await fs.writeFile(`${ctx.sourceDir}/coupe.yaml`, configYaml);
};
