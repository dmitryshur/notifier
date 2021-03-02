import { chromium } from 'playwright';

class Scraper {
  async run(url: string, script: string): Promise<boolean> {
    const browser = await chromium.launch();
    const page = await browser.newPage();
    await page.goto(url);

    // After page load, disable all network requests
    await page.route('**/*', (route) => {
      route.abort();
    });
    const fullScript = `(() => {
      ${script}
    })();`;
    const result: boolean = (await page.evaluate(fullScript)) || false;
    await browser.close();

    return result;
  }
}

export default Scraper;
