import { describe, it, expect } from "vitest";
import { normalizeServerUrl } from "../lib/url";

describe("normalizeServerUrl", () => {
  it("returns empty for blank input", () => {
    expect(normalizeServerUrl("")).toBe("");
    expect(normalizeServerUrl("   ")).toBe("");
  });

  it("preserves origin for https URLs", () => {
    expect(normalizeServerUrl("https://app.example.com")).toBe("https://app.example.com");
  });

  it("strips path/query/hash (extracts origin only)", () => {
    expect(normalizeServerUrl("https://app.example.com/dashboard?a=1#top"))
      .toBe("https://app.example.com");
  });

  it("preserves port in origin", () => {
    expect(normalizeServerUrl("https://app.example.com:8443/path"))
      .toBe("https://app.example.com:8443");
  });

  it("auto-adds https for domain names", () => {
    expect(normalizeServerUrl("app.example.com")).toBe("https://app.example.com");
  });

  it("auto-adds http for IP addresses", () => {
    expect(normalizeServerUrl("192.168.1.1")).toBe("http://192.168.1.1");
    expect(normalizeServerUrl("192.168.1.1:5099")).toBe("http://192.168.1.1:5099");
  });

  it("auto-adds http for localhost", () => {
    expect(normalizeServerUrl("localhost")).toBe("http://localhost");
    expect(normalizeServerUrl("localhost:5099")).toBe("http://localhost:5099");
  });

  it("auto-adds http for domain with explicit port", () => {
    expect(normalizeServerUrl("myserver:8080")).toBe("http://myserver:8080");
  });

  it("handles http:// prefix as-is", () => {
    expect(normalizeServerUrl("http://192.168.1.1:5099/path"))
      .toBe("http://192.168.1.1:5099");
  });

  it("handles unusual schemes by auto-prefixing https and extracting what URL can parse", () => {
    // "not://..." has no http(s) prefix, so normalizeServerUrl prepends "https://"
    // and URL constructor parses "https://not" — this is by design (user-input sanitization)
    const result = normalizeServerUrl("not://valid:url:here");
    expect(result).toContain("https://");
  });
});
