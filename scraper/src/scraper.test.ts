import Scraper from './scraper';

describe('Scraper', () => {
  test('Should display yandex.ru, find div with .services-new__list class, return true if found', async () => {
    const scraper = new Scraper();
    const script = `
      (() => {
      const el = document.querySelector('.services-new__list');
      
      return !!el
      })();
      `;
    const result = await scraper.run('https://yandex.ru', script);
    expect(result).toBe(true);
  });

  test('Should display yandex.ru, try to find a div with a random class, fail and return false', async () => {
    const scraper = new Scraper();
    const script = `
      (() => {
      const el = document.querySelector('.some-random-classname');
      
      return !!el
      })();
      `;
    const result = await scraper.run('https://yandex.ru', script);
    expect(result).toBe(false);
  });

  test('Should display yandex.ru, find weather block, compare text and return true if the word "Погода" is present', async () => {
    const scraper = new Scraper();
    const script = `
      (() => {
      const el = document.querySelector('.weather__header > h1:nth-child(1) > a:nth-child(1)');
      const text = el.text;
      
      return text === 'Погода'
      })();
      `;
    const result = await scraper.run('https://yandex.ru', script);
    expect(result).toBe(true);
  });
});
