export const SETUPS = {
  toolbarDropdown: async (page) => {
    await page.evaluate(() => {
      const bar = document.querySelector('div.flex.items-center.gap-3.px-3');
      if (!bar) return;
      bar.style.position = "relative";
      if (bar.querySelector('[data-capture-dropdown]')) return;
      const dd = document.createElement("div");
      dd.setAttribute("data-capture-dropdown", "1");
      dd.style.position = "absolute";
      dd.style.top = "44px";
      dd.style.right = "12px";
      dd.style.width = "240px";
      dd.style.background = "white";
      dd.style.border = "1px solid rgba(0,0,0,0.08)";
      dd.style.borderRadius = "12px";
      dd.style.boxShadow = "0 8px 32px rgba(0,0,0,0.12)";
      dd.style.padding = "8px";
      dd.style.zIndex = "50";
      dd.style.fontFamily = "system-ui, -apple-system, sans-serif";
      dd.innerHTML =
        '<div style="font-size:12px;font-weight:600;padding:6px 8px;color:#111">Experimental toolbar</div>' +
        '<div style="font-size:13px;padding:8px 10px;border-radius:8px;background:rgba(236,72,153,0.1);color:#ec4899;margin-bottom:6px">Model: Whisper Small · Live</div>' +
        '<div style="font-size:13px;padding:6px 10px;color:#444">Server mode: off</div>' +
        '<div style="font-size:13px;padding:6px 10px;color:#444">VAD: enabled</div>' +
        '<div style="height:1px;background:rgba(0,0,0,0.06);margin:6px 0"></div>' +
        '<div style="font-size:12px;padding:6px 10px;color:#888">Press ?view=screenshots to open the gallery</div>';
      // dark mode tweak — use app dark background #2c2b29
      if (document.documentElement.getAttribute("data-theme") === "dark" || window.matchMedia("(prefers-color-scheme: dark)").matches) {
        dd.style.background = "#2c2b29";
        dd.style.backgroundColor = "#2c2b29";
        dd.style.borderColor = "rgba(255,255,255,0.08)";
      } else {
        // light keeps CSS variable / white for screenshot consistency
        dd.style.background = "var(--color-background, white)";
      }
      bar.appendChild(dd);
    });
    await page.waitForTimeout(300);
  },

  promptExpanded: async (page) => {
    await page.evaluate(() => {
      const el = document.querySelector(".ptap-body");
      if (!el) return;
      el.focus();
      // Insert sample content via DOM (tiptap will pick up onUpdate on next transaction, but for screenshot DOM is enough)
      const sample = document.createElement("div");
      sample.innerHTML =
        "<h1>Q4 Planning Prompt</h1>" +
        "<p>Summarize the transcript below into 3 concise bullets, then draft a 2-sentence next step.</p>" +
        "<blockquote>Transcript: We shipped the new onboarding and cut time-to-first-value by 32%. The prompt library is still experimental — users love the insertion flow but want folders.</blockquote>" +
        "<ul><li>Keep tone crisp, no filler</li><li>Preserve numbers and names</li></ul>" +
        "<pre><code>variables: {{customer}} · {{quarter}}</code></pre>";
      // Clear placeholder and append
      const prose = el.querySelector(".tiptap") || el;
      // tiptap renders inside .tiptap; try both
      const target = document.querySelector(".ptap-body .tiptap") || el;
      target.innerHTML = sample.innerHTML;
      target.style.minHeight = "320px";
    });
    await page.waitForTimeout(400);
  },

  quickPrompt: async (page) => {
    await page.evaluate(() => {
      const ta = document.querySelector("textarea");
      if (ta) {
        ta.value = "Summarize the standup notes into 3 bullets and draft next steps.\n\nTranscript: shipped onboarding, cut time-to-value by 32%...";
        ta.dispatchEvent(new Event("input", { bubbles: true }));
        ta.dispatchEvent(new Event("change", { bubbles: true }));
        // trigger React state via native setter
        const proto = Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, "value");
        if (proto && proto.set) {
          const set = proto.set;
          set.call(ta, ta.value);
          ta.dispatchEvent(new Event("input", { bubbles: true }));
        }
      }
      // inject a fake history list below textarea if not present (mock has empty history)
      const root = document.querySelector(".quick-prompt-root");
      if (root && !root.querySelector("[data-capture-history]")) {
        const isHistDark = document.documentElement.getAttribute("data-theme") === "dark" || window.matchMedia("(prefers-color-scheme: dark)").matches;
        const hist = document.createElement("div");
        hist.setAttribute("data-capture-history", "1");
        if (isHistDark) {
          hist.style.cssText = "margin: 0 12px 8px; border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; overflow: hidden; background: rgba(255,255,255,0.06);";
          hist.innerHTML =
            '<div style="padding:6px 12px; font-size:10px; font-weight:600; letter-spacing:0.08em; text-transform:uppercase; color: rgba(255,255,255,0.4)">Recent — mock history</div>' +
            '<div style="padding:8px 12px; font-size:13px; border-top:1px solid rgba(255,255,255,0.08); display:flex; justify-content:space-between; gap:12px"><span style="white-space:nowrap; overflow:hidden; text-overflow:ellipsis; flex:1">Fix typo in README: accomodate → accommodate</span><span style="font-size:10px; color: rgba(255,255,255,0.4)">Today</span></div>' +
            '<div style="padding:8px 12px; font-size:13px; border-top:1px solid rgba(255,255,255,0.08); display:flex; justify-content:space-between; gap:12px"><span style="white-space:nowrap; overflow:hidden; text-overflow:ellipsis; flex:1">Draft reply to design review: keep tone crisp, no filler</span><span style="font-size:10px; color: rgba(255,255,255,0.4)">Yesterday</span></div>' +
            '<div style="padding:8px 12px; font-size:13px; border-top:1px solid rgba(255,255,255,0.08); display:flex; justify-content:space-between; gap:12px; background: rgba(236,72,153,0.12)"><span style="white-space:nowrap; overflow:hidden; text-overflow:ellipsis; flex:1">Summarize the standup notes into 3 bullets…</span><span style="font-size:10px; color: rgba(255,255,255,0.4)">2h ago</span></div>';
        } else {
          hist.style.cssText = "margin: 0 12px 8px; border: 1px solid rgba(0,0,0,0.08); border-radius: 12px; overflow: hidden; background: rgba(0,0,0,0.03);";
          hist.innerHTML =
            '<div style="padding:6px 12px; font-size:10px; font-weight:600; letter-spacing:0.08em; text-transform:uppercase; color: rgba(0,0,0,0.35)">Recent — mock history</div>' +
            '<div style="padding:8px 12px; font-size:13px; border-top:1px solid rgba(0,0,0,0.06); display:flex; justify-content:space-between; gap:12px"><span style="white-space:nowrap; overflow:hidden; text-overflow:ellipsis; flex:1">Fix typo in README: accomodate → accommodate</span><span style="font-size:10px; color: rgba(0,0,0,0.35)">Today</span></div>' +
            '<div style="padding:8px 12px; font-size:13px; border-top:1px solid rgba(0,0,0,0.06); display:flex; justify-content:space-between; gap:12px"><span style="white-space:nowrap; overflow:hidden; text-overflow:ellipsis; flex:1">Draft reply to design review: keep tone crisp, no filler</span><span style="font-size:10px; color: rgba(0,0,0,0.35)">Yesterday</span></div>' +
            '<div style="padding:8px 12px; font-size:13px; border-top:1px solid rgba(0,0,0,0.06); display:flex; justify-content:space-between; gap:12px; background: rgba(236,72,153,0.08)"><span style="white-space:nowrap; overflow:hidden; text-overflow:ellipsis; flex:1">Summarize the standup notes into 3 bullets…</span><span style="font-size:10px; color: rgba(0,0,0,0.35)">2h ago</span></div>';
        }
        const card = root.querySelector(".max-w-\\[640px\\]") || root.firstElementChild;
        if (card) {
          const footer = card.querySelector(".border-t");
          if (footer) card.insertBefore(hist, footer);
          else card.appendChild(hist);
        }
      }
    });
    await page.waitForTimeout(400);
  },
};

export async function runSetup(page, name) {
  if (!name) return;
  const fn = SETUPS[name];
  if (!fn) return;
  await fn(page);
}
