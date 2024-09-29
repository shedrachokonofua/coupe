# Coupe

An ultralight toolkit for building small, event-driven FaaS applications.

## Features

- Supported Function Languages/Runtimes:

  - [x] Rust
  - [x] TypeScript (Node.js)
  - [ ] Java (Quarkus)
  - [ ] Docker

- Supported Function Triggers:
  - [x] HTTP
  - NATS:
    - [ ] PubSub
    - [ ] Queue
    - [ ] Stream
    - [ ] KV Change
    - [ ] Object Change
    - [ ] Cron
- Supported Deployment Targets:
  - [x] Docker Compose
  - [ ] Docker Swarm
  - [ ] AWS CloudFormation
  - [ ] Kubernetes
- [x] Scale to Zero
- [x] Easy CLI
- [x] Caddy Reverse Proxy
- [ ] OpenTelemetry (OTel) Integration
- [ ] Authn/Authz powered by Keycloak

## Prerequisites

- Docker
- Docker-Compose
- Bun
- Task

## Installation

- Clone the repository
- Run `task install`

## Getting Started

- Copy the `example` directory to a new directory
- Run `coupe deploy` in the directory
- Visit `http://localhost:8080/hello` in your browser

## Commands

- [x] `coupe deploy` - Deploy the current directory
- [ ] `coupe new <name> <template>` - Create a new project from a template
- [ ] `coupe stop` - Stop running deployment for the current directory
- [ ] `coupe up` - Start stopped deployment for the current directory
- [ ] `coupe teardown` - Undeploy the current directory
