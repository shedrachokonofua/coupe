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

export const doesPathExist = async (path: string) => {
  try {
    await $`test -e ${path}`;
    return true;
  } catch (e) {
    return false;
  }
};

export const assertPath = async (path: string) => {
  if (!(await doesPathExist(path))) {
    throw new Error(`Path ${path} does not exist.`);
  }
};

export const getFunctionTemplatePath = (runtime: string, triggerType: string) =>
  Path.resolve(FUNCTION_TEMPLATES_DIR, `${runtime}/${triggerType}`);

export const getTriggerTemplatePath = (runtime: string, triggerType: string) =>
  `${getFunctionTemplatePath(runtime, triggerType)}/trigger`;

export const getHandlerTemplatePath = (runtime: string, triggerType: string) =>
  `${getFunctionTemplatePath(runtime, triggerType)}/handler`;

export const isUniqueArray = (arr: string[]) =>
  new Set(arr).size === arr.length;

export const dropStartEndSlash = (str: string) => str.replace(/^\/|\/$/g, "");
