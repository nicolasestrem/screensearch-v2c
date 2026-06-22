import typography from "@tailwindcss/typography";

/** @type {import('tailwindcss').Config} */
// Tailwind theme maps utilities onto the CSS custom properties defined in
// src/styles/tokens.css — the single source of styling truth (UI_REFERENCE §2).
//
// Color, radius, font family/size/weight, and tracking are *replaced* with the
// token set so components can't reach an off-palette hue or off-scale type. The
// default `spacing` scale is kept deliberately: its rem steps already equal the
// design spacing scale (1·2·3·4·6·8·12 = 4·8·12·16·24·32·48 px) and it backs the
// width/height/inset/max-* utilities used for structural layout. We extend it
// only with the `hit-min` accessibility alias.
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    // Replace Tailwind's default palette wholesale — only Command Deck tokens exist.
    colors: {
      transparent: "transparent",
      current: "currentColor",
      base: "var(--bg-base)",
      surface: "var(--bg-surface)",
      overlay: "var(--bg-overlay)",
      scrim: "var(--scrim)",
      line: "var(--line)",
      ink: {
        DEFAULT: "var(--ink)",
        muted: "var(--ink-muted)",
        faint: "var(--ink-faint)",
      },
      accent: {
        DEFAULT: "var(--accent)",
        wash: "var(--accent-wash)",
      },
      danger: "var(--danger)",
      warn: "var(--warn)",
      ok: "var(--ok)",
    },
    borderRadius: {
      none: "var(--radius-ribbon)",
      chip: "var(--radius-chip)",
      panel: "var(--radius-panel)",
      full: "9999px",
    },
    fontFamily: {
      display: "var(--font-display)",
      body: "var(--font-body)",
      mono: "var(--font-mono)",
    },
    fontSize: {
      display: ["var(--text-display)", { lineHeight: "1.1" }],
      title: ["var(--text-title)", { lineHeight: "1.2" }],
      subtitle: ["var(--text-subtitle)", { lineHeight: "1.3" }],
      body: ["var(--text-body)", { lineHeight: "1.5" }],
      caption: ["var(--text-caption)", { lineHeight: "1.4" }],
      data: ["var(--text-data)", { lineHeight: "1.4" }],
    },
    fontWeight: {
      regular: "var(--weight-regular)",
      medium: "var(--weight-medium)",
      semibold: "var(--weight-semibold)",
    },
    extend: {
      // Keep default spacing; add the ≥32px hit-target alias (UI_REFERENCE §7).
      spacing: {
        "hit-min": "var(--hit-min)",
      },
      letterSpacing: {
        eyebrow: "var(--tracking-eyebrow)",
      },
      zIndex: {
        base: "var(--z-base)",
        rail: "var(--z-rail)",
        overlay: "var(--z-overlay)",
        toast: "var(--z-toast)",
      },
      transitionTimingFunction: {
        ui: "var(--ease-ui)",
      },
      transitionDuration: {
        fast: "var(--motion-fast)",
        base: "var(--motion-base)",
        slow: "var(--motion-slow)",
      },
      boxShadow: {
        // The signature scan-head's halo (the one place we spend glow).
        scan: "var(--glow-scan)",
      },
      borderColor: {
        DEFAULT: "var(--line)",
      },
      // Prose (RAG answers) themed onto Command Deck tokens — UI_REFERENCE §1.
      typography: () => ({
        deck: {
          css: {
            "--tw-prose-body": "var(--ink)",
            "--tw-prose-headings": "var(--ink)",
            "--tw-prose-links": "var(--accent)",
            "--tw-prose-bold": "var(--ink)",
            "--tw-prose-counters": "var(--ink-muted)",
            "--tw-prose-bullets": "var(--ink-faint)",
            "--tw-prose-hr": "var(--line)",
            "--tw-prose-quotes": "var(--ink-muted)",
            "--tw-prose-quote-borders": "var(--line)",
            "--tw-prose-code": "var(--ink)",
            "--tw-prose-pre-code": "var(--ink)",
            "--tw-prose-pre-bg": "var(--bg-overlay)",
            "--tw-prose-th-borders": "var(--line)",
            "--tw-prose-td-borders": "var(--line)",
          },
        },
      }),
    },
  },
  plugins: [typography],
};
