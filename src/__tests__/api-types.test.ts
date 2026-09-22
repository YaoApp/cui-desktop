import { describe, it, expect } from "vitest";
import type { CloudServerInfo, WellKnownInfo, AppConf } from "../lib/api";

/**
 * Type-level smoke tests — verify the exported interfaces are structurally
 * correct. These catch accidental type regressions at build time.
 */
describe("API type structures", () => {
  it("CloudServerInfo has required fields", () => {
    const info: CloudServerInfo = {
      name: "Test Server",
      slug: "test",
      url: "https://test.example.com",
    };
    expect(info.name).toBe("Test Server");
    expect(info.slug).toBe("test");
    expect(info.url).toBe("https://test.example.com");
    expect(info.region).toBeUndefined();
    expect(info.status).toBeUndefined();
    expect(info.contact_email).toBeUndefined();
  });

  it("CloudServerInfo accepts optional fields", () => {
    const info: CloudServerInfo = {
      name: "Beijing Server",
      slug: "bj",
      url: "https://bj.yaoagents.cn",
      region: "北京",
      status: "active",
      contact_email: "admin@example.com",
    };
    expect(info.region).toBe("北京");
    expect(info.status).toBe("active");
    expect(info.contact_email).toBe("admin@example.com");
  });

  it("WellKnownInfo has nullable fields", () => {
    const info: WellKnownInfo = {
      name: null,
      version: null,
      openapi: null,
      dashboard: null,
      issuer_url: null,
    };
    expect(info.name).toBeNull();
    expect(info.webproxy).toBeUndefined();
  });

  it("AppConf has servers array", () => {
    const conf: AppConf = {
      name: "Test",
      logo: "",
      port: 5099,
      theme: { primaryColor: "#333" },
      updater: { active: false, endpoints: [], pubkey: "" },
      servers: [{ url: "https://a.com", label: "A" }],
    };
    expect(conf.servers).toHaveLength(1);
    expect(conf.servers[0].url).toBe("https://a.com");
  });
});
