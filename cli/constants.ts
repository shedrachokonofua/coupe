import os from "node:os";
import path from "node:path";

export const COUPE_DIR = path.join(os.homedir(), ".coupe");
export const STACK_DEPLOYMENT_DIR = `${COUPE_DIR}/stacks`;
export const TEMPLATES_DIR = `${COUPE_DIR}/templates`;
export const FUNCTION_TEMPLATES_DIR = `${TEMPLATES_DIR}/functions`;
export const PACKAGE_TEMPLATES_DIR = `${TEMPLATES_DIR}/packages`;
