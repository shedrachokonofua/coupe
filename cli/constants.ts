import { $ } from "bun";

export const COUPE_DIR = (await $`echo "/$HOME/.coupe"`.text()).trim();
export const STACK_DIR = `${COUPE_DIR}/stacks`;
export const TEMPLATES_DIR = `${COUPE_DIR}/templates`;
export const FUNCTION_TEMPLATES_DIR = `${TEMPLATES_DIR}/functions`;
