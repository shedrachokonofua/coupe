import { $ } from "bun";
import fs from "fs/promises";
import jsonToYaml from "json-to-pretty-yaml";
import type { Config } from "./config";
import {
  cleanFolder,
  dropStartEndSlash,
  ensurePath,
  getFunctionTemplatePath,
} from "./utils";
import { STACK_DIR } from "./constants";

interface CommandContext {
  config: Config;
  workspaceSourceDir: string;
}

export const deploy = async (ctx: CommandContext, ...params: unknown[]) => {
  const workspaceStackDir = `${STACK_DIR}/${ctx.config.name}`;
  await ensurePath(workspaceStackDir);

  const containersWithEnvFiles = new Set();

  // Prepare function build directories
  for (const f of ctx.config.functions) {
    const templateDir = getFunctionTemplatePath(f.runtime, f.trigger.type);
    const handlerSourceDir = `${ctx.workspaceSourceDir}/${f.path}`;
    const fnBuildDir = `${workspaceStackDir}/${f.name}`;
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

  const dockerComposeJson = {
    name: `coupe_stack_${ctx.config.name}`,
    services: {
      sablier: {
        container_name: `coupe_stack_${ctx.config.name}_sablier`,
        image: "coupe_sablier", // Had to make it cold-start faster(https://github.com/acouvreur/sablier/issues/282), cloned it and made some changes -> pc/projects/sablier
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

  for (const f of ctx.config.functions) {
    dockerComposeJson.services[f.containerName] = {
      container_name: f.containerName,
      build: `./${f.name}`,
      labels: ["sablier.enable=true", `sablier.group=${f.containerName}`],
      profiles: ["function"],
    };
    if (containersWithEnvFiles.has(f.containerName)) {
      dockerComposeJson.services[f.containerName].env_file = [".env"];
    }
  }

  const dockerComposeYaml = jsonToYaml.stringify(dockerComposeJson);
  await fs.writeFile(
    `${workspaceStackDir}/docker-compose.yaml`,
    dockerComposeYaml
  );

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
                    session_duration ${f.idle_timeout_sec}s

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
  await fs.writeFile(`${workspaceStackDir}/Caddyfile`, caddyFileContent);
  // Format the Caddyfile
  await $`docker run --rm -v ${workspaceStackDir}:/app caddy:2.6.4-with-sablier caddy fmt --overwrite /app/Caddyfile`;

  // Build functions docker images
  await $`docker-compose -f ${workspaceStackDir}/docker-compose.yaml --profile function create --build --force-recreate`;

  // Rebuild platform docker containers
  await $`docker-compose -f ${workspaceStackDir}/docker-compose.yaml --profile platform up --build -d`;

  await $`echo "Deployment complete!"`;
};

export const start = async (ctx: CommandContext, ...params: unknown[]) => {
  await $`echo "Starting..."`;
};

export const stop = async (ctx: CommandContext, ...params: unknown[]) => {
  await $`echo "Stopping..."`;
};
