// UI memory-leak check. Repeats the main flows (open/close addon pages,
// search, switch tabs and settings) and, after forcing a full garbage
// collection, prints JS heap, DOM nodes and event listeners per round.
// Those numbers should stay flat across rounds.
//
//   VITE_MOCK=1 npx vite build --outDir dist-mock && npx vite preview --outDir dist-mock --port 4173
//   URL=http://localhost:4173 node scripts/leak-test.cjs [rounds] [cyclesPerRound]
//
// Needs the `playwright` package and a Chromium browser.
const { chromium } = require("playwright");

(async () => {
  const url = process.env.URL || "http://localhost:4173";
  const rounds = Number(process.argv[2] || 5);
  const perRound = Number(process.argv[3] || 20);
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("HeapProfiler.enable");
  // Locators (not waitForSelector) so the test itself holds no element handles.
  const ready = (sel) => page.locator(sel).first().waitFor();
  await page.goto(url);
  await ready(".row-item");

  async function measure(label) {
    for (let i = 0; i < 3; i++) await cdp.send("HeapProfiler.collectGarbage");
    // A heap snapshot forces a full V8 + DOM collection.
    await cdp.send("HeapProfiler.takeHeapSnapshot", { reportProgress: false });
    const heap = await cdp.send("Runtime.getHeapUsage");
    const dom = await cdp.send("Memory.getDOMCounters");
    console.log(
      `${label.padEnd(9)} heap=${(heap.usedSize / 1048576).toFixed(2)}MB nodes=${dom.nodes} listeners=${dom.jsEventListeners}`,
    );
    return heap.usedSize;
  }

  const sizes = [await measure("start")];
  for (let r = 1; r <= rounds; r++) {
    for (let i = 0; i < perRound; i++) {
      await page.locator(".row-item").nth(i % 10).click();
      await ready(".page h1");
      await page.locator('.tab:has-text("Files")').click();
      await page.locator('button:has-text("Back")').click();
      await page.fill("input[type=search]", i % 2 ? "quest" : "bag");
      await page.keyboard.press("Enter");
      await page.waitForTimeout(200);
      await page.fill("input[type=search]", "");
      await page.keyboard.press("Enter");
      await ready(".row-item");
      await page.locator(".all").click();
      await page.locator('.tab:has-text("My Addons")').click();
      await ready(".th");
      await page.locator('.tab:has-text("Browse")').click();
      await page.locator(".rail-btn[title=Settings]").click();
      await page.locator(".rail-btn[title^=World]").click();
      await ready(".row-item");
    }
    sizes.push(await measure(`round ${r}`));
  }
  const growth = (sizes[sizes.length - 1] - sizes[1]) / 1048576;
  console.log(`heap growth after round 1: ${growth.toFixed(2)}MB, page errors: ${errors.length}`);
  await browser.close();
  process.exit(errors.length ? 1 : 0);
})();
