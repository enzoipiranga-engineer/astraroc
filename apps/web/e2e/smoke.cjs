// Teste de fumaça no navegador: abre o app, remove a junta da coifa do exemplo PET,
// confere o aviso e a separação no voo, voa o exemplo de competição e checa o layout
// no celular. Uso: pnpm --filter @astra/web preview & node apps/web/e2e/smoke.cjs [url] [pasta-das-capturas]
const { chromium } = require('playwright');
const dir = process.argv[3] || '.';
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1400, height: 1000 }, deviceScaleFactor: 1 });
  await p.addInitScript(() => { localStorage.setItem('astra.tour', '1'); localStorage.setItem('astra.lang', 'pt'); });
  const errs = [];
  p.on('pageerror', e => errs.push('pageerror ' + e.message));
  p.on('console', m => m.type() === 'error' && errs.push('console ' + m.text()));
  await p.goto(process.argv[2] || 'http://localhost:4173');
  await p.waitForSelector('.rocket-view svg', { timeout: 15000 });
  await p.waitForFunction(() => !document.querySelector('.busy'), null, { timeout: 15000 });
  console.log('issues PET:', await p.locator('.issues').innerText());
  await p.screenshot({ path: dir + '/1-pet.png', fullPage: true });
  // Remove a junta da coifa.
  await p.locator('.part-list button').first().click();
  await p.locator('.joint-list button', { hasText: 'Garrafa PET' }).first().click();
  await p.locator('.joint-list .danger').click();
  await p.waitForTimeout(300);
  await p.waitForFunction(() => !document.querySelector('.busy'), null, { timeout: 15000 });
  console.log('issues sem junta:', (await p.locator('.issues').innerText()).slice(0, 600));
  await p.locator('header button.launch').click();
  await p.waitForSelector('.results', { timeout: 20000 });
  console.log('resultado:', (await p.locator('.results').innerText()).slice(0, 1200));
  await p.screenshot({ path: dir + '/2-coifa-solta.png', fullPage: true });
  // Exemplo de competição.
  await p.selectOption('.project .tpl', 'competicao-500m');
  await p.waitForFunction(() => !document.querySelector('.busy'), null, { timeout: 15000 });
  await p.locator('header button.launch').click();
  await p.waitForSelector('.results', { timeout: 20000 });
  console.log('competição:', (await p.locator('.results .cards').innerText()).replace(/\n/g, ' | '));
  await p.screenshot({ path: dir + '/3-competicao.png', fullPage: true });
  // Celular.
  const m = await b.newPage({ viewport: { width: 390, height: 844 } });
  await m.goto(process.argv[2] || 'http://localhost:4173');
  await m.waitForSelector('.rocket-view svg', { timeout: 15000 });
  const overflow = await m.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  console.log('overflow horizontal no celular:', overflow);
  await m.screenshot({ path: dir + '/4-celular.png', fullPage: false });
  console.log('erros:', errs);
  await b.close();
})().catch(e => { console.error(e); process.exit(1); });
