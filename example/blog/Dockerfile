FROM node:22-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM node:22-alpine AS production
WORKDIR /app
COPY package*.json ./
RUN npm ci --only=production && npm cache clean --force
COPY --from=builder /app/dist ./dist
COPY --from=builder /app/server.js ./server.js
RUN addgroup -g 1001 -S nodejs && \
    adduser -S astro -u 1001
RUN chown -R astro:nodejs /app
USER astro
CMD ["node", "server.js"]
