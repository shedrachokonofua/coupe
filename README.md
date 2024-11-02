# Coupe

A tiny FaaS framework for building event-driven applications.

## Features

- Function Runtimes:

  - Rust
  - TypeScript (Node.js)
  - Sveltekit (Node.js + TypeScript)
  - Java (GraalVM)
  - Clojure (Babashka + GraalVM)

- Triggers:

| Trigger/Language | Rust | TypeScript (Node.js) | Java | Clojure | Sveltekit |
| --- | --- | --- | --- | --- | --- |
| HTTP | ✓ | ✓ | ☐ | ☐ | ☐ |
| Pub Sub | ✓ | ☐ | ☐ | ☐ | N/A |
| Queue | ✓ | ☐ | ☐ | ☐ | N/A |
| Stream | ✓ | ☐ | ☐ | ☐ | N/A |
| Cron | ☐ | ☐ | ☐ | ☐ | N/A |

- Message Brokers:

  - [x] NATS
  - [ ] Redis

- Build Targets:

  - [x] Docker Compose
  - [ ] AWS CloudFormation

- [x] Scale to Zero
- [x] Easy CLI
- [x] Caddy Reverse Proxy
- [ ] OpenTelemetry Integration: Tracing, Metrics, Logging
- [ ] Pyroscope Integration for continuous profiling
- [ ] Auth(n/z): Bring Your Own Identity Provider (BYOIDP) using OAuth 2.0, RBAC, ABAC

## Prerequisites

- Docker
- Docker-Compose
- Deno
- Task

## Installation

- Clone the repository
- Run `task install`

## Getting Started

- Copy the `example` directory to a new directory
- Run `coupe deploy` in the directory
- Visit `http://localhost:8080/hello` in your browser

## Commands

- [x] `coupe init <name> [directory]` - Initialize a new project
- [x] `coupe add <name> <runtime> <trigger>` - Add a new function to the current project with template
- [x] `coupe scaffold` - Restore the necessary directories and packages for the project.
- [x] `coupe deploy` - Deploy the current directory
- [ ] `coupe stop` - Stop running deployment for the current directory
- [ ] `coupe up` - Start stopped deployment for the current directory
- [ ] `coupe teardown` - Undeploy the current directory
