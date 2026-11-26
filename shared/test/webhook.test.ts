// T29.9: the same rules as `agent/core/src/webhook.rs`: `http://` only to this computer and the local network.
import { describe, expect, it } from "vitest";
import { validateActions, webhookUrlProblem } from "../src/validate";

describe("the address of a webhook", () => {
  it("lets https go anywhere and http stay local", () => {
    for (const ok of [
      "http://127.0.0.1:8123/api", "http://localhost/x", "http://192.168.1.20/on", "http://10.0.0.5", "http://172.16.3.4:80", "http://[::1]/",
      "http://[fd00::1]/", "http://nas/share", "http://homeassistant.local:8123/api/webhook/abc", "https://example.com/hook", "https://8.8.8.8/",
    ]) expect(webhookUrlProblem(ok), ok).toBeNull();
    for (const no of [
      "http://example.com/", "http://8.8.8.8/", "http://172.32.0.1/", "http://100.64.0.1/", "http://[2001:db8::1]/", "http://[::ffff:8.8.8.8]/",
      "http://192.168.1.2.example.com/", "http://127.0.0.1.nip.io/",
    ]) expect(webhookUrlProblem(no), no).toMatch(/^http:\/\/ is only/);
    for (const bad of ["ftp://192.168.1.1/", "file:///c:/x", "javascript:alert(1)", "https://user:pass@example.com/", "http://", "", "relative/path"]) {
      expect(webhookUrlProblem(bad), bad).not.toBeNull();
    }
  });

  it("refuses a body on GET and headers that belong to the connection", () => {
    const issues = validateActions({
      version: 1,
      actions: { hook: { name: "Hook", type: "http", method: "GET", url: "https://example.com/", body: "x", headers: { Host: "evil", "X-Ok": "1", "X-Bad": "a\r\nb" } } },
    });
    expect(issues.map((i) => i.path).sort()).toEqual(["/actions/hook/body", "/actions/hook/headers/Host", "/actions/hook/headers/X-Bad"]);
  });
});
