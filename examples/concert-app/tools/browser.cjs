/* Host-selected real-browser checks; no model text is executed. */
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { chromium } = require(process.env.GDD_PLAYWRIGHT || '/Users/mikkelfreltoftkrogsholm/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright');
const args = Object.fromEntries(process.argv.slice(2).reduce((pairs, value, index, all) => index % 2 ? pairs : [...pairs, [value.slice(2), all[index + 1]]], []));
const output = path.resolve(args.output);
fs.mkdirSync(output, {recursive: true});
const cases = [], errors = [], requests = [];
let browser;
async function check(name, operation) { await operation(); cases.push({name, result: 'PASS'}); }
async function visibleText(page, selector, text) {
  await page.waitForFunction(({selector, text}) => document.querySelector(selector)?.textContent.includes(text), {selector, text});
}
async function login(page, user, password = user + '-demo') {
  await page.locator('#username').fill(user);
  await page.locator('#password').fill(password);
  await page.locator('#sign-in').click();
  if (password === user + '-demo') {
    await visibleText(page, '#session-status', `Signed in as ${user[0].toUpperCase() + user.slice(1)}`);
    await page.waitForFunction(() => !document.querySelector('#refresh-reservations').disabled);
  }
}
async function logout(page) {
  await page.locator('#sign-out').click();
  await visibleText(page, '#session-status', "You're signed out");
}
const button = (page, concert) => page.locator(`button[data-concert-id="${concert}"]`);
async function reserve(page, concert) {
  await button(page, concert).click();
  await visibleText(page, '#status-message', 'is reserved.');
  await page.waitForFunction(() => !document.querySelector('#refresh-reservations').disabled);
}
async function state(page) { return page.evaluate(async () => (await fetch('/api/reservations')).json()); }

(async () => {
  browser = await chromium.launch({executablePath: process.env.GDD_CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true});
  const context = await browser.newContext({viewport: {width: 1280, height: 900}, reducedMotion: 'reduce'});
  const page = await context.newPage();
  page.on('pageerror', error => errors.push(error.message));
  page.on('request', request => {
    if (request.method() === 'POST' && new URL(request.url()).pathname === '/api/reservations') requests.push(JSON.parse(request.postData()));
  });
  await page.goto(args.url);
  await check('public program and invalid credentials', async () => {
    await visibleText(page, '#session-status', "You're signed out");
    assert.equal(await page.locator('.concert-card').count(), 3);
    await login(page, 'alice', 'wrong');
    await visibleText(page, '#error-message', "isn't right");
    assert.equal(await page.locator('#reservations-section').isVisible(), false);
    await login(page, 'alice');
  });
  if (args.phase === 'a') {
    await check('lost-response-retry-does-not-duplicate', async () => {
      let lost = false;
      await page.route('**/api/reservations', async route => {
        if (route.request().method() === 'POST' && !lost) {
          lost = true;
          const committed = await route.fetch();
          assert.equal(committed.status(), 200);
          await route.abort('failed');
        } else await route.continue();
      });
      await button(page, 'midnight').click();
      await visibleText(page, '#error-message', "couldn't confirm your reservation");
      await page.waitForFunction(() => [...document.querySelectorAll('button')].some(b => b.textContent === 'Retry reservation' && !b.disabled));
      assert.equal((await state(page)).active_count, 1);
      await button(page, 'midnight').click();
      await visibleText(page, '#status-message', 'is reserved.');
      await page.waitForFunction(() => !document.querySelector('#refresh-reservations').disabled);
      assert.equal((await state(page)).active_count, 1);
      assert.equal(requests[0].request_key, requests[1].request_key);
      await page.unroute('**/api/reservations');
    });
    await check('phase A permits three across concerts and cancellation preserves history', async () => {
      await reserve(page, 'northern');
      await reserve(page, 'matinee');
      const before = await state(page);
      assert.equal(before.active_count, 3);
      assert.equal(before.active_limit, null);
      const first = before.reservations.find(r => r.concert_id === 'midnight');
      await page.locator(`[data-reservation-id="${first.id}"] button`).click();
      await visibleText(page, '#status-message', 'is cancelled.');
      await page.waitForFunction(() => !document.querySelector('#refresh-reservations').disabled);
      const after = await state(page);
      assert.equal(after.active_count, 2);
      assert.equal(after.reservations.length, 3);
      assert.equal(after.reservations.find(r => r.id === first.id).state, 'cancelled');
    });
  } else {
    await check('phase B preserves A history, uses server limit and permits cancellation at limit', async () => {
      const before = await state(page);
      assert.equal(before.active_count, 2);
      assert.equal(before.active_limit, 2);
      assert.equal(before.reservations.length, 3);
      await visibleText(page, '#reservation-summary', '2 of 2');
      assert.equal(await button(page, 'midnight').isDisabled(), true);
      const forbidden = await page.evaluate(async () => {
        const response = await fetch('/api/reservations', {method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({concert_id:'midnight',request_key:'browser-b-unbound'})});
        return {status: response.status, body: await response.json()};
      });
      assert.equal(forbidden.status, 409);
      assert.equal(forbidden.body.error.code, 'ActiveReservationLimitReached');
      const target = before.reservations.find(r => r.state === 'reserved' && r.concert_id === 'northern');
      await page.locator(`[data-reservation-id="${target.id}"] button`).click();
      await visibleText(page, '#status-message', 'is cancelled.');
      await page.waitForFunction(() => !document.querySelector('#refresh-reservations').disabled);
      await reserve(page, 'northern');
      assert.equal((await state(page)).active_count, 2);
      assert.equal((await state(page)).reservations.length, 4);
    });
    // A fresh independent context exercises the same transport control under B.
    await check('lost-response-retry-does-not-duplicate', async () => {
      const isolated = await browser.newContext();
      const retryPage = await isolated.newPage();
      await retryPage.goto(args.url);
      await visibleText(retryPage, '#session-status', "You're signed out");
      await login(retryPage, 'bob');
      const before = (await state(retryPage)).active_count;
      const keys = [];
      let lost = false;
      await retryPage.route('**/api/reservations', async route => {
        if (route.request().method() === 'POST') {
          keys.push(JSON.parse(route.request().postData()).request_key);
          if (!lost) {
            lost = true;
            const committed = await route.fetch();
            assert.equal(committed.status(), 200);
            await route.abort('failed');
            return;
          }
        }
        await route.continue();
      });
      await button(retryPage, 'midnight').click();
      await visibleText(retryPage, '#error-message', "couldn't confirm your reservation");
      await retryPage.waitForFunction(() => [...document.querySelectorAll('button')].some(b => b.textContent === 'Retry reservation' && !b.disabled));
      await button(retryPage, 'midnight').click();
      await visibleText(retryPage, '#status-message', 'is reserved.');
      await retryPage.waitForFunction(() => !document.querySelector('#refresh-reservations').disabled);
      assert.equal((await state(retryPage)).active_count, before + 1);
      assert.equal(keys[0], keys[1]);
      await retryPage.unroute('**/api/reservations');
      // The server committed, but an unknown 500 response cannot confirm outcome.
      const unknownKeys = [];
      let failedResponse = false;
      await retryPage.route('**/api/reservations', async route => {
        if (route.request().method() === 'POST') {
          unknownKeys.push(JSON.parse(route.request().postData()).request_key);
          if (!failedResponse) {
            failedResponse = true;
            const committed = await route.fetch();
            assert.equal(committed.status(), 200);
            await route.fulfill({status:500, contentType:'application/json', body:JSON.stringify({error:{code:'InternalError',message:'The request could not be completed.'}})});
            return;
          }
        }
        await route.continue();
      });
      await button(retryPage, 'northern').click();
      await visibleText(retryPage, '#error-message', "couldn't confirm your reservation");
      await retryPage.waitForFunction(() => [...document.querySelectorAll('button')].some(b => b.textContent === 'Retry reservation' && !b.disabled));
      assert.equal((await state(retryPage)).active_count, before + 2);
      await button(retryPage, 'northern').click();
      await visibleText(retryPage, '#status-message', 'is reserved.');
      await retryPage.waitForFunction(() => !document.querySelector('#refresh-reservations').disabled);
      assert.equal((await state(retryPage)).active_count, before + 2);
      assert.equal(unknownKeys[0], unknownKeys[1]);
      cases.push({name:'postcommit-unknown500-retains-key-at-limit', result:'PASS', transport:'Real upstream commit; browser control substitutes unknown500 response. Separate real HTTP test induces serialization failure.'});
      await isolated.close();
    });
  }
  await check('identity switch hides old history and suppresses a late private response', async () => {
    // Hold an actual UI refresh, then change identity before its old response arrives.
    let oldRelease, oldCapturedResolve, oldDeliveredResolve;
    const oldHeld = new Promise(resolve => oldRelease = resolve);
    const oldCaptured = new Promise(resolve => oldCapturedResolve = resolve);
    const oldDelivered = new Promise(resolve => oldDeliveredResolve = resolve);
    let once = false;
    const oldIds = await page.locator('[data-reservation-id]').evaluateAll(rows => rows.map(row => row.dataset.reservationId));
    await page.route('**/api/reservations', async route => {
      if (route.request().method() === 'GET' && !once) {
        once = true; const response = await route.fetch(); oldCapturedResolve(); await oldHeld; await route.fulfill({response}); oldDeliveredResolve();
      } else await route.continue();
    });
    await page.locator('#refresh-reservations').click();
    await oldCaptured;
    await logout(page);
    assert.equal(await page.locator('#reservation-history').textContent(), '');
    await login(page, 'bob');
    const bobBefore = await state(page);
    oldRelease();
    await oldDelivered;
    await page.unroute('**/api/reservations');
    await page.waitForTimeout(100);
    assert.equal((await state(page)).active_count, bobBefore.active_count);
    for (const id of oldIds) assert.equal(await page.locator(`[data-reservation-id="${id}"]`).count(), 0);
    await logout(page);
    await login(page, 'alice');
  });
  if (args.phase === 'b') {
    await check('shared-cookie session changes reject stale reserve, history and logout', async () => {
      // Leave room in Alice's allowance so the stale reserve is actually clickable.
      await page.locator('#reservation-history button').first().click();
      await visibleText(page, '#status-message', 'is cancelled.');
      await page.waitForFunction(() => !document.querySelector('#refresh-reservations').disabled);
      const second = await context.newPage();
      async function switchOtherToBob() {
        await second.goto(args.url);
        await visibleText(second, '#session-status', 'Signed in as Alice');
        await logout(second);
        await login(second, 'bob');
      }
      await switchOtherToBob();
      const bobBefore = (await state(second)).active_count;
      await button(page, 'midnight').click();
      await visibleText(page, '#session-status', "You're signed out");
      assert.equal(await page.locator('#reservation-history').textContent(), '');
      assert.equal((await state(second)).active_count, bobBefore);
      await login(page, 'alice');
      await switchOtherToBob();
      await page.locator('#refresh-reservations').click();
      await visibleText(page, '#session-status', "You're signed out");
      assert.equal(await page.locator('#reservation-history').textContent(), '');
      await login(page, 'alice');
      await switchOtherToBob();
      await page.locator('#sign-out').click();
      await visibleText(page, '#error-message', 'other session was preserved');
      assert.equal(await page.locator('#sign-in').isDisabled(), false);
      const current = await second.evaluate(async () => (await fetch('/api/session')).json());
      assert.equal(current.principal.id, 'bob');
      assert.equal((await state(second)).active_count, bobBefore);
      await login(page, 'alice');
      await reserve(page, 'midnight');
      // Preserve the first logout's identity expectation across a lost response.
      let lostLogout = false;
      const logoutExpectations = [];
      await page.route('**/api/logout', async route => {
        logoutExpectations.push(route.request().headers()['x-concert-principal']);
        if (!lostLogout) {
          lostLogout = true;
          const committed = await route.fetch();
          assert.equal(committed.status(), 200);
          await route.abort('failed');
        } else await route.continue();
      });
      await page.locator('#sign-out').click();
      await visibleText(page, '#session-status', 'Sign-out not yet confirmed');
      await second.goto(args.url);
      await visibleText(second, '#session-status', "You're signed out");
      await login(second, 'bob');
      await page.locator('#sign-out').click();
      await visibleText(page, '#error-message', 'other session was preserved');
      assert.deepEqual(logoutExpectations, ['alice', 'alice']);
      assert.equal((await second.evaluate(async () => (await fetch('/api/session')).json())).principal.id, 'bob');
      assert.equal(await page.locator('#sign-in').isDisabled(), false);
      await page.unroute('**/api/logout');
      await login(page, 'alice');
      cases.push({name:'uncertain-logout-retry-preserves-original-identity-expectation',result:'PASS'});
      await second.close();
    });
  }
  await check('keyboard, labelled forms, 320px and 200 percent layout', async () => {
    await logout(page);
    await page.locator('#username').focus();
    await page.keyboard.type('alice');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'password');
    await page.keyboard.type('alice-demo');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'sign-in');
    await page.keyboard.press('Enter');
    await visibleText(page, '#session-status', 'Signed in as Alice');
    await page.waitForFunction(() => !document.querySelector('#refresh-reservations').disabled);
    assert.equal(await page.evaluate(() => window.matchMedia('(prefers-reduced-motion: reduce)').matches), true);
    await page.screenshot({path: path.join(output, 'desktop.png'), fullPage: true});
    for (const width of [320, 640]) {
      await page.setViewportSize({width, height: 900});
      await page.evaluate(scale => document.documentElement.style.zoom = scale, width === 640 ? '2' : '1');
      const metrics = await page.evaluate(() => ({viewport: innerWidth, content: document.documentElement.scrollWidth, minControl: Math.min(...[...document.querySelectorAll('button,input')].filter(el => el.getClientRects().length).map(el => el.getBoundingClientRect().height))}));
      assert.ok(metrics.content <= metrics.viewport + 1, JSON.stringify(metrics));
      assert.ok(metrics.minControl >= 44, JSON.stringify(metrics));
    }
    await page.evaluate(() => document.documentElement.style.zoom = '1');
    await page.setViewportSize({width: 320, height: 900});
    await page.screenshot({path: path.join(output, 'mobile.png'), fullPage: true});
    assert.equal(await page.locator('[role="status"]').count(), 1);
    assert.equal(await page.locator('[role="alert"]').count(), 1);
  });
  assert.deepEqual(errors, []);
  const receipt = {format:'concert-browser-receipt/0.1', executed:true, result:'PASS', phase:args.phase, browser:await browser.version(), url:args.url, cases, errors, reserve_requests:requests, negative_controls:['lost-response-retry-does-not-duplicate'], final_history:await state(page)};
  fs.writeFileSync(path.join(output, 'browser.run.json'), JSON.stringify(receipt,null,2)+'\n');
  console.log(JSON.stringify({result:receipt.result,cases:cases.length,browser:receipt.browser}));
})().catch(error => {
  fs.writeFileSync(path.join(output, 'browser.failure.json'), JSON.stringify({result:'FAIL',phase:args.phase,cases,error:error.stack},null,2)+'\n');
  console.error(error.stack); process.exitCode = 1;
}).finally(async () => { if (browser) await browser.close(); });
