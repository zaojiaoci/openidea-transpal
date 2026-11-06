# Issue 2 草稿 — 清单外域名的按需授权（net:on-use）

标题：feat(plugins): ask-on-use consent for network hosts not declared in the manifest

## Body

### Problem

The capability system gates `net:` per host, from the static manifest list. That is the right default, but it leaves no room for **endpoints the user configures at runtime** — a growing plugin category:

- AI plugins that accept any OpenAI-compatible endpoint (DeepSeek, Kimi, GLM, a relay, a local Ollama on `localhost:11434` …);
- self-hosted tools where the instance URL is inherently user-specific (Gitea, Forgejo, a company wiki).

Concretely, with today's rules a plugin that lets the user type an endpoint URL can only talk to hosts pre-declared in its manifest:

- `net:*` (or any prefix wildcard) does not parse — host labels allow only alphanumerics and `-`, so at most `*.example.com` subdomain wildcards exist;
- the user cannot add a host themselves either: `setCapability` rejects capabilities the plugin does not declare ("plugin … does not declare net:api.moonshot.cn").

So in our plugin (TransPal Translate), the default DeepSeek endpoint and `localhost` (declared for Ollama) work, but a user pointing the endpoint setting at Kimi or a relay gets `net:api.moonshot.cn: Denied` with no way to allow it.

### Proposal

Ask-on-use consent, per host, remembered like other grants:

- when a plugin's `fetch` targets a host that is neither declared nor granted, the IDE shows the standard consent dialog — "TransPal Translate wants to connect to api.moonshot.cn. Allow?" — with Allow once / always / deny;
- an allowed host is stored in the ledger as `(plugin, net:host)` and shows up in Settings › Plugins › Permissions alongside declared ones, so it stays visible and revocable;
- a denied host fails the fetch with the usual `Denied` error, no dialog spam.

Why per-host consent rather than "grant whatever the endpoint setting points to": setting defaults are written by the plugin author, so a capability that follows a setting is effectively an author-chosen blanket grant. Asking per host keeps every new domain a explicit user decision, which is the same trust model as the existing consent dialog — just triggered by use instead of install.

The pieces for this already exist: the consent dialog and ledger plumbing (including the `ask_on_use` migration flag, which shows the pattern was anticipated), and the permission UI. The main new surface is a decision + dialog at fetch time when `require(Net(host))` fails, instead of an immediate error.

### Motivating plugin

[TransPal Translate] — three translation sources today; the AI source is OpenAI-compatible by design but can only reach its default host. Ask-on-use would make "any endpoint the user configures" actually true.
