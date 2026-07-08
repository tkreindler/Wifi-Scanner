// Quick visual iteration: screenshots dev server (ui-fixture) at app width.
const { chromium, webkit } = require("@playwright/test");

function launchBrowser() {
  if (process.platform === "win32") {
    return chromium.launch({ channel: "msedge" });
  }
  if (process.platform === "darwin") {
    return webkit.launch();
  }
  return chromium.launch();
}

(async () => {
  const b = await launchBrowser();
  const p = await b.newPage({ viewport: { width: 1000, height: 720 } });
  const errs = []; p.on("pageerror", (e) => errs.push(e.message));
  await p.goto("http://localhost:5173");
  await p.waitForSelector("body[data-ready='1']", { timeout: 8000 }).catch(() => {});
  await p.waitForTimeout(900);
  await p.screenshot({ path: "../tmp/shot.png", fullPage: true });
  console.log("errs", errs);
  await b.close();
})();
