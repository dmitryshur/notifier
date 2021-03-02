import amqp, { ConsumeMessage } from 'amqplib';
import type { Channel, Connection } from 'amqplib';

export type Exchanges = 'scraper' | 'bot';

export interface Scrape {
  Scrape: {
    id: string;
    chat_id: string;
    url: string;
    script: string;
  };
}

export function isScrape(msg: any): msg is Scrape {
  const obj = JSON.parse(msg);

  if (obj.Scrape) {
    return ['id', 'chat_id', 'url', 'script'].every((prop) => prop in obj.Scrape);
  }

  return false;
}

// TODO missing chat_id
export interface Notify {
  Notify: {
    id: string;
    chat_id: string;
  };
}

export type Messages = Scrape | Notify;

export interface Consumer {
  (msg: ConsumeMessage | null): void;
}

function delay(duration: number) {
  return new Promise((resolve) => setTimeout(resolve, duration));
}

class Broker {
  #addr: string;
  #channel: Channel | null = null;
  #exchanges: Array<Exchanges> = [];

  constructor(addr: string) {
    this.#addr = addr;
  }

  async init() {
    let connection: Connection;
    let retries = 1;
    let interval = 1000;

    for (let i = 0; i < 5; i++) {
      try {
        connection = await amqp.connect(this.#addr);
      } catch (error) {
        console.log(`Trying to connect to Rabbit. attempt ${retries++}`);
        await delay(interval);
        interval *= 2;

        if (i === 4) {
          throw new Error(`Connection error. addr: ${this.#addr}. error: ${error}`);
        }
      }
    }

    try {
      this.#channel = await connection!.createChannel();
    } catch (error) {
      throw new Error(`Channel error. ${this.#addr} channel. error: ${error}`);
    }
  }

  async publish(exchange: Exchanges, msg: Messages) {
    if (!this.#channel) return;

    try {
      if (!this.#exchanges.includes(exchange)) {
        await this.#channel.assertExchange(exchange, 'direct', { durable: true });
        this.#exchanges.push(exchange);
      }

      this.#channel.publish(exchange, exchange, Buffer.from(JSON.stringify(msg)));
    } catch (error) {
      throw new Error(`Publish error. exchange: ${exchange}. message: ${msg}. error: ${error}`);
    }
  }

  async subscribe(exchange: Exchanges, consumer: Consumer) {
    if (!this.#channel) return;

    try {
      if (!this.#exchanges.includes(exchange)) {
        await this.#channel.assertExchange(exchange, 'direct', { durable: true });
        this.#exchanges.push(exchange);
      }

      const queue = await this.#channel.assertQueue('', { exclusive: true });
      await this.#channel.bindQueue(queue.queue, exchange, exchange);
      await this.#channel.consume(queue.queue, consumer);
    } catch (error) {
      throw new Error(`Subscribe error. exchange: ${exchange}. error: ${error}`);
    }
  }
}

export default Broker;
