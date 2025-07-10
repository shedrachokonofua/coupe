import express from "express";
import pino from "pino";
import pinoHttp from "pino-http";
import { handler as astroHandler } from "./dist/server/entry.mjs";

const logger = pino({
  level: process.env.LOG_LEVEL || "info",
});

const app = express();
const PORT = process.env.PORT || 3000;

app.use(pinoHttp({ logger }));

app.get("/health", (req, res) => {
  const stats = getStats();
  req.log.info({ stats }, "Health check requested");
  res.json({ status: "OK", ...stats });
});

const serverStartTime = new Date();
let uniqueIPs = new Set();

function getStats() {
  const uptimeMs = Date.now() - serverStartTime.getTime();
  const hours = Math.floor(uptimeMs / (1000 * 60 * 60));
  const minutes = Math.floor((uptimeMs % (1000 * 60 * 60)) / (1000 * 60));
  const seconds = Math.floor((uptimeMs % (1000 * 60)) / 1000);

  return {
    uniqueVisitors: uniqueIPs.size,
    uptime: `${hours}h ${minutes}m ${seconds}s`,
  };
}

app.use((req, res, next) => {
  const ip = req.ip || "unknown";
  const isNewVisitor = !uniqueIPs.has(ip);

  if (isNewVisitor) {
    uniqueIPs.add(ip);
    req.log.info(
      { ip, totalUniqueVisitors: uniqueIPs.size },
      "New unique visitor"
    );
  }

  const stats = getStats();
  req.log.debug({ ip, stats }, "Request processing");

  const locals = { stats };
  astroHandler(req, res, next, locals);
});

app.use(express.static("dist/client"));

app.listen(PORT, () => {
  logger.info(
    {
      port: PORT,
      env: process.env.NODE_ENV || "development",
      logLevel: logger.level,
    },
    "Server started successfully"
  );
});
