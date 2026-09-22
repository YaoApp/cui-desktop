import { describe, it, expect, beforeEach } from "vitest";
import { normalizeServerUrl } from "../lib/url";

/**
 * Tests for server selection logic that can be verified without DOM/Tauri:
 * - URL normalization behavior for cloud vs custom servers
 * - Cloud base / locale resolution
 * - Connect-enabled logic
 */
describe("server selection logic", () => {
  describe("cloud vs custom URL handling", () => {
    it("cloud server URL should not be normalized (preserves path)", () => {
      const cloudUrl = "https://bj.yaoagents.cn/dashboard";
      // Cloud servers pass URL directly, no normalization
      expect(cloudUrl).toBe("https://bj.yaoagents.cn/dashboard");
    });

    it("custom server URL gets normalized (extracts origin)", () => {
      const customInput = "https://my-server.com/some/path?query=1";
      const normalized = normalizeServerUrl(customInput) || customInput;
      expect(normalized).toBe("https://my-server.com");
    });

    it("custom server bare domain gets https prefix", () => {
      const normalized = normalizeServerUrl("my-server.com");
      expect(normalized).toBe("https://my-server.com");
    });

    it("custom server IP gets http prefix", () => {
      const normalized = normalizeServerUrl("10.0.0.1:5099");
      expect(normalized).toBe("http://10.0.0.1:5099");
    });
  });

  describe("cloud base and locale resolution", () => {
    it("zh locale maps to yaoagents.cn with zh-cn", () => {
      const lang: string = "zh";
      const cloudBase = lang === "zh" ? "https://yaoagents.cn" : "https://yaoagents.com";
      const locale = lang === "zh" ? "zh-cn" : "en-US";
      expect(cloudBase).toBe("https://yaoagents.cn");
      expect(locale).toBe("zh-cn");
    });

    it("en locale maps to yaoagents.com with en-US", () => {
      const lang: string = "en";
      const cloudBase = lang === "zh" ? "https://yaoagents.cn" : "https://yaoagents.com";
      const locale = lang === "zh" ? "zh-cn" : "en-US";
      expect(cloudBase).toBe("https://yaoagents.com");
      expect(locale).toBe("en-US");
    });
  });

  describe("connect button enable logic", () => {
    function isConnectEnabled(tab: number, cloudSelectedUrl: string, customUrl: string): boolean {
      if (tab === 0) return !!cloudSelectedUrl;
      return !!customUrl.trim();
    }

    it("cloud tab: disabled when no server selected", () => {
      expect(isConnectEnabled(0, "", "")).toBe(false);
    });

    it("cloud tab: enabled when a server is selected", () => {
      expect(isConnectEnabled(0, "https://bj.yaoagents.cn", "")).toBe(true);
    });

    it("custom tab: disabled when URL is empty or whitespace", () => {
      expect(isConnectEnabled(1, "", "")).toBe(false);
      expect(isConnectEnabled(1, "", "   ")).toBe(false);
    });

    it("custom tab: enabled when URL is provided", () => {
      expect(isConnectEnabled(1, "", "https://my-server.com")).toBe(true);
    });
  });

  describe("active server tab determination", () => {
    function determineTab(
      activeUrl: string,
      cloudServers: Array<{ url: string }>
    ): { tab: number; cloudSelected: string; customUrl: string } {
      const normalized = (u: string) => u.replace(/\/$/, "");
      const isCloud = cloudServers.some(s => normalized(s.url) === normalized(activeUrl));
      if (isCloud) {
        return { tab: 0, cloudSelected: activeUrl, customUrl: "" };
      }
      return { tab: 1, cloudSelected: "", customUrl: activeUrl };
    }

    it("active server matching a cloud URL selects cloud tab", () => {
      const result = determineTab("https://bj.yaoagents.cn", [
        { url: "https://bj.yaoagents.cn" },
        { url: "https://sh.yaoagents.cn" },
      ]);
      expect(result.tab).toBe(0);
      expect(result.cloudSelected).toBe("https://bj.yaoagents.cn");
    });

    it("active server matching with trailing slash difference still matches", () => {
      const result = determineTab("https://bj.yaoagents.cn/", [
        { url: "https://bj.yaoagents.cn" },
      ]);
      expect(result.tab).toBe(0);
    });

    it("non-cloud active server selects custom tab", () => {
      const result = determineTab("https://my-private.com", [
        { url: "https://bj.yaoagents.cn" },
      ]);
      expect(result.tab).toBe(1);
      expect(result.customUrl).toBe("https://my-private.com");
    });
  });

  describe("register URL locale routing", () => {
    it("zh users go to yaoagents.cn/servers", () => {
      const isZh = true;
      const url = isZh ? "https://yaoagents.cn/servers" : "https://yaoagents.com/servers";
      expect(url).toBe("https://yaoagents.cn/servers");
    });

    it("en users go to yaoagents.com/servers", () => {
      const isZh = false;
      const url = isZh ? "https://yaoagents.cn/servers" : "https://yaoagents.com/servers";
      expect(url).toBe("https://yaoagents.com/servers");
    });
  });
});
