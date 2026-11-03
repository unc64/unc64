// Prints an HTML file to PDF with Chromium (Playwright): node render.js IN.html OUT.pdf
const { chromium } = require('playwright');
(async () => {
  const [, , input, output] = process.argv;
  const browser = await chromium.launch();
  const page = await browser.newPage();
  await page.goto('file://' + input, { waitUntil: 'load' });
  await page.evaluate(() => document.fonts.ready);
  await page.pdf({ path: output, preferCSSPageSize: true, printBackground: true, outline: true, tagged: true });
  await browser.close();
})();
