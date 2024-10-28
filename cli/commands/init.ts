import Path from "node:path";
import fse from "fs-extra";
import jsonToYaml from "json-to-pretty-yaml";

export const init = async (params: string[]) => {
  if (params.length === 0) {
    throw new Error("Invalid number of arguments");
  }

  const [name, dir = "."] = params;
  const sourceDir = Path.resolve(Deno.cwd(), dir);

  if (await fse.pathExists(`${sourceDir}/coupe.yaml`)) {
    throw new Error("Coupe project already exists.");
  }

  const config = jsonToYaml.stringify({
    name,
    functions: [],
  });

  await fse.outputFile(`${sourceDir}/coupe.yaml`, config);

  console.log(`Coupe project created at ${sourceDir}`);
};
