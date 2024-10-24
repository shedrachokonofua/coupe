# Coupe

A tiny FaaS framework for building event-driven applications.

## Features

- Function Languages/Runtimes:

  - [x] Rust
  - [x] TypeScript (Node.js)
  - [ ] Sveltekit (Node.js + TypeScript)
  - [ ] Java (GraalVM)

- Function Triggers:
  - [x] HTTP
  - [ ] Async:
    - [x] PubSub
    - [ ] Queue
    - [ ] Stream
    - [ ] Cron
- Message Brokers:
  - [x] NATS
  - [ ] Redis
- Build Targets:
  - [x] Docker Compose
  - [ ] AWS CloudFormation
- [x] Scale to Zero
- [x] Easy CLI
- [x] Caddy Reverse Proxy
- [ ] OpenTelemetry Integration
- [ ] Authn/Authz: OAuth 2.0, OIDC, ABAC, RBAC

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
- [x] `coupe add <name> <runtime> <trigger>` - Add a new function to the current project with template
- [ ] `coupe new <name>` - Initialize a new project
- [ ] `coupe stop` - Stop running deployment for the current directory
- [ ] `coupe up` - Start stopped deployment for the current directory
- [ ] `coupe teardown` - Undeploy the current directory
