import winston from 'winston';
import Broker, { isScrape } from './broker';
import type { Scrape, Notify } from './broker';

const logger = winston.createLogger({
  level: 'info',
  format: winston.format.json(),
  defaultMeta: { service: 'scraper' },
  transports: [new winston.transports.Console(), new winston.transports.File({ filename: 'logs.log' })],
});

// TODO get env vars
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

  try {
    await broker.subscribe('scraper', (msg) => {
      const content = msg?.content.toString();

      if (isScrape(content)) {
        const message: Scrape = JSON.parse(content);
        // TODO handle msg
        logger.info(`msg received. ${message}`);
      }
    });
  } catch (error) {
    logger.warn(`${error.name}. ${error.message}. ${error.stack}`);
  }
})();
