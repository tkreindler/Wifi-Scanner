import { test, expect } from "@playwright/test";

test("renders three band spectrum panels incl. 6 GHz", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator("body")).toHaveAttribute("data-ready", "1");
  await expect(page.getByTestId("iface")).toContainText("fixture");
  await expect(page.locator(".chip")).toHaveCount(3);
  await expect(page.locator('.chip[data-band="band6"]')).toContainText("4");
  await expect(page.getByTestId("spec24")).toBeVisible();
  await expect(page.getByTestId("spec5")).toBeVisible();
  await expect(page.getByTestId("spec6")).toBeVisible();
  await expect(page.locator("#rows")).toContainText("Workstation-6E");
  await expect(page.locator("#rows tr").first()).toBeVisible();
});
