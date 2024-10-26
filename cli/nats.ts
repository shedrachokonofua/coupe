import {
  connect,
  NatsConnection,
  JetStreamManager,
  StreamConfig,
  StreamInfo,
  NatsError,
  ConsumerConfig,
  ConsumerInfo,
} from "nats";

export class NatsClient {
  private constructor(
    private readonly conn: NatsConnection,
    private readonly jsm: JetStreamManager
  ) {}

  static async connect(port: number): Promise<NatsClient> {
    const nc = await connect({
      servers: [`nats://localhost:${port}`],
    });
    const jsm = await nc.jetstreamManager();
    return new NatsClient(nc, jsm);
  }

  async close() {
    await this.conn.close();
  }

  async getOrCreateStream(config: Partial<StreamConfig>): Promise<StreamInfo> {
    if (!config.name) {
      throw new Error("Stream name is required");
    }

    try {
      return await this.jsm.streams.info(config.name);
    } catch (error) {
      if ((error as NatsError).code === "404") {
        return await this.jsm.streams.add(config);
      }
      throw error;
    }
  }

  async getOrCreateConsumer(
    stream: string,
    config: Partial<ConsumerConfig>
  ): Promise<ConsumerInfo> {
    if (!config.durable_name) {
      throw new Error("Durable name is required");
    }

    try {
      return await this.jsm.consumers.info(stream, config.durable_name);
    } catch (error) {
      if ((error as NatsError).code === "404") {
        return await this.jsm.consumers.add(stream, config);
      }
      throw error;
    }
  }
}
