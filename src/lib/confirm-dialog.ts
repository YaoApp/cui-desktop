import { t } from "./i18n";

/** Show a custom HTML confirm dialog — replaces browser native confirm(). */
export function confirmDialog(message: string): Promise<boolean> {
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.className = "modal-overlay";
    overlay.innerHTML = `
      <div class="modal-card confirm-dialog-card">
        <div class="confirm-dialog-body">${escHtml(message)}</div>
        <div class="modal-actions">
          <button class="btn-ghost btn-sm" id="confirm-cancel">${escHtml(t("app.cancel"))}</button>
          <button class="btn-main btn-sm" id="confirm-ok">${escHtml(t("app.confirm"))}</button>
        </div>
      </div>
    `;
    document.body.appendChild(overlay);

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close(false);
      if (e.key === "Enter") close(true);
    };
    document.addEventListener("keydown", onKey);

    function close(result: boolean) {
      document.removeEventListener("keydown", onKey);
      overlay.remove();
      resolve(result);
    }

    overlay.querySelector("#confirm-ok")!.addEventListener("click", () => close(true));
    overlay.querySelector("#confirm-cancel")!.addEventListener("click", () => close(false));
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) close(false);
    });
  });
}

/** Show a custom HTML alert dialog — replaces browser native alert(). */
export function alertDialog(message: string): Promise<void> {
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.className = "modal-overlay";
    overlay.innerHTML = `
      <div class="modal-card confirm-dialog-card">
        <div class="confirm-dialog-body">${escHtml(message)}</div>
        <div class="modal-actions">
          <button class="btn-main btn-sm" id="alert-ok">${escHtml(t("app.confirm"))}</button>
        </div>
      </div>
    `;
    document.body.appendChild(overlay);

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" || e.key === "Enter") close();
    };
    document.addEventListener("keydown", onKey);

    function close() {
      document.removeEventListener("keydown", onKey);
      overlay.remove();
      resolve();
    }

    overlay.querySelector("#alert-ok")!.addEventListener("click", close);
  });
}

function escHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}
