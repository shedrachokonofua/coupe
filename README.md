# Coupe

Coupe is a lightweight, language-agnostic FaaS(Function as a Service) platform for building simple, event-driven applications.

## Features

- **Event-driven** - Supports HTTP, message queues, streams, and timers
- **Simple contract** - Each function is an HTTP server in a container
- **Language agnostic** - Write functions in any language
- **Scale to zero** - Functions sleep when idle, wake on demand
- **OpenTelemetry native** - Distributed tracing and metrics baked in
- **OpenAPI generation** - Auto-generate API specs from function schemas
- **MCP integration** - Functions become AI tools automatically
- **Enterprise security** - OAuth 2.0 and policy-based authorization

## Getting Started

This guide explains how to run the provided Astro blog example.

### 1. Clone the Repository

```sh
git clone https://github.com/shedrachokonofua/coupe.git
cd coupe
```

### 2. Install the CLI

Use the provided `task` command to install the `coupe-cli`:

```sh
task install:cli
```

### 3. Build Docker Images

Before deploying, you need to build the Docker images for the sentinel and the example function.

```sh
task release:sentinel
task release:example:blog
```

### 4. Deploy the Stack

From the root of the repository, run the `deploy` command. This will start the Coupe sentinel and deploy the function.

```sh
coupe-cli deploy --path example/coupe.yaml
```

The sentinel will now be running and listening for requests on port **52345**.

### 5. Invoke the Function

You can now visit the Astro blog in your browser or use `curl`:

```sh
curl http://localhost:52345/
```

## Configuration

Coupe is configured using a `coupe.yaml` file. This file declaratively defines your entire service stack, from functions to triggers and external services.

Below is a comprehensive example followed by a detailed reference for all configuration options.

### Example

```yaml
name: my-awesome-app
version: 0.1.0
description: "An awesome application powered by Coupe"

sentinel:
  port: 8080
  otel_endpoint: "http://localhost:4317"
  registry:
    url: "docker.io"
    namespace: "my-namespace"

identity:
  provider:
    type: "auth0"
    domain: "your-tenant.auth0.com"
    client_id: "your-client-id"
    client_secret: "your-client-secret"
    audience: "your-api-audience"

brokers:
  my-nats-broker:
    type: nats
    connection: "nats://localhost:4222"

queues:
  email-queue:
    broker: my-nats-broker
    subject: "emails"

streams:
  order-stream:
    broker: my-nats-broker
    stream: "orders"
    subjects:
      - "orders.created"
      - "orders.updated"

openapi:
  definitions:
    Product:
      type: "object"
      properties:
        id:
          type: "string"
        name:
          type: "string"

functions:
  console:
    image: my-namespace/console:latest
    trigger:
      type: http
      path: "*"
      method: "Get"
      auth:
        type: web
        protected_routes: ["/admin"]
        policies: ["admin-only"]

  get-products:
    image: my-namespace/products-api:latest
    handler_port: 3000
    trigger:
      type: http
      path: "/products"
      method: "Get"
      schema:
        responses:
          "200":
            description: "A list of products"
            content:
              application/json:
                schema:
                  type: "array"
                  items:
                    $ref: "#/definitions/Product"
      auth:
        type: jwt
        scopes: ["read:products"]
        policies: []

  process-order:
    image: my-namespace/order-processor:latest
    trigger:
      type: stream
      stream: "order-stream"

  send-welcome-email:
    image: my-namespace/email-sender:latest
    trigger:
      type: queue
      queue: "email-queue"

  cron-job:
    image: my-namespace/cron-job:latest
    trigger:
      type: timer
      schedule: "0 0 * * *" # Every day at midnight
```

### Top-Level Fields

| Key           | Type       | Description                                                                                                |
| ------------- | ---------- | ---------------------------------------------------------------------------------------------------------- |
| `name`        | `string`   | **Required.** The name of your service stack. Used for naming Docker resources.                            |
| `version`     | `string`   | The version of your service.                                                                               |
| `description` | `string`   | A short description of your service.                                                                       |
| `sentinel`    | `Sentinel` | Configuration for the Coupe sentinel (the main proxy).                                                     |
| `identity`    | `Identity` | Configures an identity provider for authentication.                                                        |
| `brokers`     | `map`      | A map of message brokers (e.g., NATS) to be used by functions.                                             |
| `queues`      | `map`      | Defines named queues that functions can subscribe to.                                                      |
| `streams`     | `map`      | Defines named streams that functions can subscribe to.                                                     |
| `openapi`     | `OpenApi`  | Provides OpenAPI definitions that can be referenced by your functions to generate a service specification. |
| `functions`   | `map`      | **Required.** A map of all the functions in your service.                                                  |

### `sentinel`

| Key             | Type                | Description                                                                                       |
| --------------- | ------------------- | ------------------------------------------------------------------------------------------------- |
| `port`          | `integer`           | The port the sentinel listens on. Defaults to `52345`.                                            |
| `otel_endpoint` | `string`            | The OpenTelemetry collector gRPC endpoint for traces and metrics (e.g., `http://localhost:4317`). |
| `registry`      | `ContainerRegistry` | Specifies a container registry to pull function images from.                                      |

### `sentinel.registry`

| Key         | Type     | Description                                            |
| ----------- | -------- | ------------------------------------------------------ |
| `url`       | `string` | The URL of the container registry (e.g., `docker.io`). |
| `namespace` | `string` | The namespace or organization within the registry.     |

### `identity`

| Key        | Type               | Description                               |
| ---------- | ------------------ | ----------------------------------------- |
| `provider` | `IdentityProvider` | Configures the identity provider details. |

### `identity.provider`

| Key             | Type     | Description                                  |
| --------------- | -------- | -------------------------------------------- |
| `type`          | `string` | The type of provider (e.g., `auth0`).        |
| `domain`        | `string` | The domain of your identity provider tenant. |
| `client_id`     | `string` | The client ID for your application.          |
| `client_secret` | `string` | The client secret for your application.      |
| `audience`      | `string` | The audience identifier for your API.        |

### `brokers`

Defines a map of message brokers. Each key is a broker name.

| Key          | Type     | Description                                              |
| ------------ | -------- | -------------------------------------------------------- |
| `type`       | `string` | **Required.** The type of broker. Currently only `nats`. |
| `connection` | `string` | **Required.** The connection string for the broker.      |

**Example**

```yaml
brokers:
  my-nats-broker:
    type: nats
    connection: "nats://localhost:4222"
```

### `queues`

Defines a map of queues, which reference a configured broker.

| Key       | Type     | Description                                              |
| --------- | -------- | -------------------------------------------------------- |
| `broker`  | `string` | **Required.** The name of a configured broker.           |
| `subject` | `string` | **Required.** The subject/topic the queue consumes from. |

**Example**

```yaml
queues:
  email-queue:
    broker: my-nats-broker
    subject: "emails"
```

### `streams`

Defines a map of streams, which reference a configured broker.

| Key             | Type     | Description                                                         |
| --------------- | -------- | ------------------------------------------------------------------- |
| `broker`        | `string` | **Required.** The name of a configured broker.                      |
| `stream`        | `string` | **Required.** The name of the stream to consume from.               |
| `subjects`      | `array`  | **Required.** A list of subjects to subscribe to within the stream. |
| `consumer_name` | `string` | An optional durable consumer name.                                  |

**Example**

```yaml
streams:
  order-stream:
    broker: my-nats-broker
    stream: "orders"
    subjects:
      - "orders.created"
      - "orders.updated"
```

### `openapi`

Allows you to define reusable OpenAPI schemas.

| Key           | Type  | Description                                                                |
| ------------- | ----- | -------------------------------------------------------------------------- |
| `definitions` | `map` | A map of OpenAPI schema definitions that can be referenced from functions. |

### `functions`

A map where each key is a function name and the value is a `Function` object.

| Key            | Type      | Description                                                                            |
| -------------- | --------- | -------------------------------------------------------------------------------------- |
| `image`        | `string`  | **Required.** The Docker image for the function.                                       |
| `trigger`      | `Trigger` | **Required.** How the function is invoked.                                             |
| `handler_port` | `integer` | The port the function's HTTP server listens on inside the container. Defaults to `80`. |
| `scaling`      | `Scaling` | Configuration for function scaling behavior.                                           |

### `functions.trigger`

A function must have exactly one trigger.

| Key    | Type     | Description                                                                      |
| ------ | -------- | -------------------------------------------------------------------------------- |
| `type` | `string` | **Required.** The type of trigger. Can be `http`, `queue`, `stream`, or `timer`. |

#### `http` Trigger

| Key      | Type       | Description                                                                                                       |
| -------- | ---------- | ----------------------------------------------------------------------------------------------------------------- |
| `path`   | `string`   | **Required.** The URL path to trigger the function. Use `*` for a catch-all.                                      |
| `method` | `string`   | The HTTP method (`Get`, `Post`, `Put`, `Delete`, `Patch`, `Any`). Defaults to `Any`.                              |
| `schema` | `object`   | An OpenAPI Operation Object describing the request and response. Can use `$ref` to link to `openapi.definitions`. |
| `auth`   | `HttpAuth` | Optional authentication rules for the endpoint.                                                                   |

#### `queue` Trigger

| Key     | Type     | Description                                          |
| ------- | -------- | ---------------------------------------------------- |
| `queue` | `string` | **Required.** The name of the queue to consume from. |

#### `stream` Trigger

| Key      | Type     | Description                                           |
| -------- | -------- | ----------------------------------------------------- |
| `stream` | `string` | **Required.** The name of the stream to consume from. |

#### `timer` Trigger

| Key        | Type     | Description                                                                         |
| ---------- | -------- | ----------------------------------------------------------------------------------- |
| `schedule` | `string` | **Required.** A cron expression for when to run the function (e.g., `"0 0 * * *"`). |

### `functions.trigger.auth`

Configures authentication for an `http` trigger.

| Key    | Type     | Description                                         |
| ------ | -------- | --------------------------------------------------- |
| `type` | `string` | **Required.** The auth type. Can be `jwt` or `web`. |

#### `jwt` Auth

Validates a JWT token from the `Authorization` header.

| Key        | Type    | Description                                                   |
| ---------- | ------- | ------------------------------------------------------------- |
| `scopes`   | `array` | A list of required scopes that must be present in the token.  |
| `policies` | `array` | A list of policies to evaluate for the request (coming soon). |

#### `web` Auth

A generic authentication mechanism for web apps, often involving cookies or sessions.

| Key                | Type    | Description                                                   |
| ------------------ | ------- | ------------------------------------------------------------- |
| `protected_routes` | `array` | A list of URL paths that require authentication.              |
| `policies`         | `array` | A list of policies to evaluate for the request (coming soon). |

### `functions.scaling`

| Key                     | Type      | Description                                                                                           |
| ----------------------- | --------- | ----------------------------------------------------------------------------------------------------- |
| `session_duration`      | `integer` | How long an idle function's container stays alive before being stopped (in seconds). Defaults to 300. |
| `health_check_interval` | `integer` | The interval in seconds to perform health checks on a running function. Defaults to 10.               |
