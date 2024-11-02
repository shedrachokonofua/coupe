import { IncomingMessage } from "http";

export const handler = async (request: IncomingMessage): Promise<Response> => {
  return new Response(
    JSON.stringify({
      message: "Hello, world!",
    }),
    {
      headers: { "content-type": "application/json" },
    }
  );
};
