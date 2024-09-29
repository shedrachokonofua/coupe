# Stage 1: Build Stage
FROM node:20-alpine AS builder

# Set working directory
WORKDIR /app

# Copy package.json and package-lock.json
COPY trigger/package*.json ./trigger/
COPY handler/package*.json ./handler/

# Install dependencies separately for caching purposes
RUN cd trigger && npm install
RUN cd handler && npm install

# Copy the entire project (both folders)
COPY trigger ./trigger
COPY handler ./handler

# Compile the TypeScript code (this will include handler in the bundle)
RUN cd trigger && npm run build

# Stage 2: Production Image
FROM node:20-alpine AS runner

# Set working directory in the production container
WORKDIR /app

# Copy only the bundled output from the build stage
COPY --from=builder /app/trigger/build ./build

# Copy production node_modules
COPY --from=builder /app/trigger/node_modules ./node_modules

# Set NODE_ENV to production for optimization
ENV NODE_ENV=production

# Expose the port
EXPOSE 80

# Command to run the app
CMD ["node", "build"]
