import { $ } from "execa";

export const COUPE_DIR = (await $`echo "/$HOME/.coupe"`).stdout.trim();
export const STACK_DEPLOYMENT_DIR = `${COUPE_DIR}/stacks`;
export const TEMPLATES_DIR = `${COUPE_DIR}/templates`;
export const FUNCTION_TEMPLATES_DIR = `${TEMPLATES_DIR}/functions`;
