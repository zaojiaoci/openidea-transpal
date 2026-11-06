# Issue 1 草稿 — 编辑器浮层扩展点

标题：feat(plugins): plugin-contributed editor popups (hover provider / peek panel)

## Body

### Problem

Plugins currently have no way to show rich content anchored to text in the editor. The extension points available today are menus, keymaps, status bar items, dock views (webviews), custom editors and diagnostics — none of them can open a small panel next to the text the user is looking at.

Translation plugins are the canonical case (IntelliJ's translation plugins all work this way): after translating a selection, the result should appear right beside the source text, ideally with structure (translation, split words, a copy button). Today a plugin's only option that ends up *near* the text is publishing a diagnostic and letting the user hover it, which has three hard limits:

- the message is plain text — no structure, no actions;
- it is listed in the Problems view, where it is not a problem;
- the hover is pointer-driven: if the diagnostic is published while the pointer already rests on the word (exactly what happens when the command ran from the context menu), no tooltip appears until the pointer moves again.

What we ship today (TransPal Translate, a translation plugin): the result goes to four channels — a dock view, the status bar, a notification and a diagnostic hint. Even together they don't reproduce the "answer where the question was asked" experience.

### Proposal

Two levels, either of which alone would unlock this:

1. **Hover provider** — plugins already have `contributes.providers` for interactive definition/completion/line-marker requests; add a `hover` request that returns text (or markdown) for a position/range, rendered by the existing CodeMirror hover infrastructure the LSP already uses (`src/editor/lsp/hover.ts`). This fixes the text-only and trigger limits.

2. **Peek panel** — a programmatic API the plugin calls when it has a result, e.g. `host::show_peek(editor, range, html)` opening a small floating panel anchored at the range, dismissed like a hover. This is the translation-plugin shape: click *Translate*, the panel opens by itself at the selection, rich content and all.

Level 1 reuses what is already there and matches the existing providers pattern; level 2 is the more general primitive (any command could pop a panel next to what it acted on). If only one is feasible, level 2 covers more cases; level 1 is the smaller change.

### Motivating plugin

[TransPal Translate] — translate selection / explain identifiers / generate names. The dock view and status bar already work; the missing piece is exactly this.
