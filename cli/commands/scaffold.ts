import fse from "fs-extra";
import type { CommandContext } from "../config.ts";
import { PACKAGE_TEMPLATES_DIR } from "../constants.ts";

const ensureGitIgnoreContains = async (dir: string, content: string) => {
  const gitignorePath = `${dir}/.gitignore`;
  await fse.ensureFile(gitignorePath);
  const gitignore = await fse.readFile(gitignorePath, "utf-8");
  const lines = new Set(gitignore.split("\n"));
  lines.add(content);
  await fse.outputFile(gitignorePath, Array.from(lines).join("\n"));
};

const getSubDirectories = async (dir: string) => {
  const files = await fse.readdir(dir, { withFileTypes: true });
  return files.filter((f) => f.isDirectory()).map((f) => f.name);
};

export const scaffoldRuntimePackages = async (
  ctx: CommandContext,
  runtime: string
) => {
  const runtimePackages = await getSubDirectories(
    `${PACKAGE_TEMPLATES_DIR}/${runtime}`
  );
  for (const pkg of runtimePackages) {
    const sourceRuntimePackageDir = `${ctx.sourceDir}/packages/${runtime}`;
    const sourcePackageDir = `${sourceRuntimePackageDir}/${pkg}`;

    if (!(await fse.pathExists(sourcePackageDir))) {
      await ensureGitIgnoreContains(sourceRuntimePackageDir, pkg);
      await fse.copy(
        `${PACKAGE_TEMPLATES_DIR}/${runtime}/${pkg}`,
        sourcePackageDir
      );
    }
  }
};

export const scaffold = async (ctx: CommandContext) => {
  await fse.ensureDir(`${ctx.sourceDir}/functions`);
  await fse.ensureDir(`${ctx.sourceDir}/packages`);

  const runtimes = new Set(ctx.config.functions.map((f) => f.runtime));
  for (const runtime of runtimes) {
    await scaffoldRuntimePackages(ctx, runtime);
  }
};
