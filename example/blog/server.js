import express from "express";
import { handler as astroHandler } from "./dist/server/entry.mjs";

const app = express();
const PORT = process.env.PORT || 3000;

const serverStartTime = new Date();
let uniqueIPs = new Set();

function getStats() {
  const uptimeMs = Date.now() - serverStartTime.getTime();
  const hours = Math.floor(uptimeMs / (1000 * 60 * 60));
  const minutes = Math.floor((uptimeMs % (1000 * 60 * 60)) / (1000 * 60));

  return {
    uniqueVisitors: uniqueIPs.size,
    uptime: `${hours}h ${minutes}m`,
  };
}

app.use((req, res, next) => {
  const ip = req.ip || "unknown";

  if (!uniqueIPs.has(ip)) {
    uniqueIPs.add(ip);
  }

  const locals = { stats: getStats() };
  astroHandler(req, res, next, locals);
});

app.use(express.static("dist/client"));

app.listen(PORT, () => {
  console.log(`Server running on http://localhost:${PORT}`);
});
