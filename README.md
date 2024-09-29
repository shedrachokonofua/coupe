# Coupe

An ultralight toolkit for building small, nimble FaaS applications.

## Features

- [x] HTTP triggers
- [x] CLI for easy deployment and management
- [x] Docker-compose deployment target
- [x] Scale to zero
- [x] Caddy integration for http routing
- [ ] NATS JetStream storage (PubSub, KV buckets, Streams, Object storage)
- [ ] NATS-powered fn-triggers (PubSub, Queue, Stream, KV change, Object change)
- [ ] OpenTelemetry (OTel) integration
- [ ] Static site hosting
- [ ] Authentication and Authorization with Keycloak

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
