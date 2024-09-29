import http from "node:http";
import { handler } from "handler/src";

async function collectStream(readableStream: ReadableStream<Uint8Array>) {
  const reader = readableStream.getReader();
  const chunks = [];

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
  }

  return Buffer.concat(chunks);
}

http
  .createServer(async (request, serverResponse) => {
    const response = await handler(request);
    const headers: http.OutgoingHttpHeaders = {};
    response.headers.forEach((value, key) => {
      headers[key] = value;
    });
    serverResponse.writeHead(response.status, headers);

    if (response.body) {
      const bodyContent = await collectStream(response.body);
      serverResponse.end(bodyContent);
    } else {
      serverResponse.end(response.body);
    }
  })
  .listen(80);
