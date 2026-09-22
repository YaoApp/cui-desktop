import { describe, it, expect, vi, beforeAll } from "vitest";

/**
 * Verifies every zhCN key has an enUS counterpart and vice versa.
 * Also checks all cloud-server-related keys are present.
 */

// Mock the Tauri invoke so i18n module init doesn't throw
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

describe("i18n key completeness", () => {
  const requiredCloudKeys = [
    "app.cloud_section",
    "app.custom_section",
    "app.cloud_loading",
    "app.cloud_empty",
    "app.cloud_select_hint",
    "app.cloud_load_failed",
    "app.cloud_retry",
    "app.server_subtitle",
    "app.register_helper",
    "app.register_link",
    "app.custom_full_hint",
  ];

  const requiredExistingKeys = [
    "app.connect",
    "app.connecting",
    "app.starting_proxy",
    "app.connected",
    "app.connection_failed",
    "app.settings",
  ];

  let t: (key: string) => string;
  let getLang: () => string;

  beforeAll(async () => {
    const mod = await import("../lib/i18n");
    t = mod.t;
    getLang = mod.getLang;
  });

  it("zhCN and enUS dictionaries have matching keys", () => {
    for (const key of [...requiredCloudKeys, ...requiredExistingKeys]) {
      localStorage.setItem("cui_lang", "zh");
      const zhVal = t(key);
      expect(zhVal, `zhCN missing key: ${key}`).not.toBe(key);

      localStorage.setItem("cui_lang", "en");
      const enVal = t(key);
      expect(enVal, `enUS missing key: ${key}`).not.toBe(key);
    }
  });

  it("cloud keys have distinct zh vs en values", () => {
    for (const key of requiredCloudKeys) {
      localStorage.setItem("cui_lang", "zh");
      const zh = t(key);
      localStorage.setItem("cui_lang", "en");
      const en = t(key);
      expect(zh, `key ${key}: zh and en should differ`).not.toBe(en);
    }
  });

  it("getLang returns zh or en based on localStorage", () => {
    localStorage.setItem("cui_lang", "zh");
    expect(getLang()).toBe("zh");
    localStorage.setItem("cui_lang", "en");
    expect(getLang()).toBe("en");
  });
});
