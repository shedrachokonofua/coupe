import { $ } from "bun";
import Path from "path";
import { FUNCTION_TEMPLATES_DIR } from "./constants";

export const createFolderIfNotExists = async (folder: string) => {
  await $`mkdir -p ${folder}`;
};

export const ensurePath = async (path: string) => {
  if (path === "." || path === "/" || path === "") {
    return;
  }

  const parts = path.split("/");
  let currentPath = ".";
  for (const part of parts) {
    currentPath += `/${part}`;
    await createFolderIfNotExists(currentPath);
  }
};

export const deleteFolderIfExists = async (folder: string) => {
  await $`rm -rf ${folder}`;
};

export const cleanFolder = async (folder: string) => {
  await deleteFolderIfExists(folder);
  await createFolderIfNotExists(folder);
};

export const assertPath = async (path: string) => {
  try {
    await $`test -e ${path}`;
  } catch (e) {
    console.error(`Path does not exist: ${path}`);
    process.exit(1);
  }
};

export const getFunctionTemplatePath = (runtime: string, triggerType: string) =>
  Path.resolve(FUNCTION_TEMPLATES_DIR, `${runtime}/${triggerType}`);

export const getTriggerTemplatePath = (runtime: string, triggerType: string) =>
  `${getFunctionTemplatePath(runtime, triggerType)}/trigger`;

export const isUniqueArray = (arr: string[]) =>
  new Set(arr).size === arr.length;

export const dropStartEndSlash = (str: string) => str.replace(/^\/|\/$/g, "");
