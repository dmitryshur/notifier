import winston from 'winston';
import Broker, { isScrape } from './broker';
import Scraper from './scraper';
import type { Scrape, Notify } from './broker';

const logger = winston.createLogger({
  level: 'info',
  format: winston.format.json(),
  defaultMeta: { service: 'scraper' },
  transports: [new winston.transports.Console()],
});

(async function main() {
  const rabbitAddress = process.env.RABBIT_ADDRESS;
  if (!rabbitAddress) {
    logger.error("Can't find RABBIT_ADDRESS env variable");
    process.exit(1);
  }

  const broker = new Broker(rabbitAddress);

  try {
    await broker.init();
  } catch (error) {
    logger.error(`${error.name}. ${error.message}. ${error.stack}`);
    throw error;
  }

  const scraper = new Scraper();
  try {
    await broker.subscribe('scraper', async (msg) => {
      const content = msg?.content.toString();

      if (isScrape(content)) {
        const message: Scrape = JSON.parse(content);
        const isSuccess = await scraper.run(message.Scrape.url, message.Scrape.script);

        // TODO switch to redis
        // TODO handle /list command in bot
        if (isSuccess) {
          logger.info(`success. send message`);
          const brokerMsg: Notify = {
            Notify: {
              id: message.Scrape.id,
              chat_id: message.Scrape.chat_id,
              url: message.Scrape.url,
            },
          };

          await broker.publish('bot', brokerMsg);
        } else {
          logger.info('not success');
        }
      }
    });
  } catch (error) {
    logger.warn(`${error.name}. ${error.message}. ${error.stack}`);
  }
})();
