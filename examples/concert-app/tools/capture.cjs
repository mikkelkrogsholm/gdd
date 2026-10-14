/* Fresh browser presentation captures, separate from the original execution receipt. */
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(process.env.GDD_PLAYWRIGHT || '/Users/mikkelfreltoftkrogsholm/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright');
const args = Object.fromEntries(process.argv.slice(2).reduce((pairs,value,index,all) => index % 2 ? pairs : [...pairs,[value.slice(2),all[index+1]]],[]));
(async () => {
  const browser = await chromium.launch({executablePath:process.env.GDD_CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',headless:true});
  try {
    const captures=[];
    for (const [name,width] of [['desktop',1280],['mobile',320]]) {
      const context=await browser.newContext({viewport:{width,height:900},reducedMotion:'reduce'});
      const page=await context.newPage();
      await page.goto(args.url);
      await page.waitForFunction(() => !document.querySelector('#sign-in').disabled);
      await page.locator('#username').fill('alice');
      await page.locator('#password').fill('alice-demo');
      await page.locator('#sign-in').click();
      await page.waitForFunction(() => document.querySelector('#session-status').textContent==='Signed in as Alice' && !document.querySelector('#refresh-reservations').disabled);
      await page.evaluate(() => document.activeElement.blur());
      await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      const metrics=await page.evaluate(() => ({viewport:innerWidth,content:document.documentElement.scrollWidth,height:document.documentElement.scrollHeight,summary:document.querySelector('#reservation-summary').textContent}));
      if (metrics.content>width+1) throw new Error('Unexpected page overflow');
      const file=name+'.clean.png';
      await page.screenshot({path:path.join(args.output,file),fullPage:true});
      captures.push({file,metrics,fresh_context:true,zoom_history:false});
      await context.close();
    }
    fs.writeFileSync(path.join(args.output,'clean-captures.json'),JSON.stringify({executed:true,result:'PASS',browser:await browser.version(),url:args.url,notice:'Actual fresh-context presentation captures after the original browser run. Original raw screenshots are preserved; their post-zoom full-page mobile capture showed compositing artifacts.',captures},null,2)+'\n');
    console.log('Fresh desktop and mobile captures PASS');
  } finally { await browser.close(); }
})().catch(error => {console.error(error.stack);process.exitCode=1;});
