# YOMNOVA AI Assistant

An optional, **suggest-only** programming assistant inside the Playground. It explains real compiler diagnostics,
explains selected code, tutors YOMNOVA syntax, and proposes corrections. It uses [OpenRouter](https://openrouter.ai/docs)
so you can choose among many models. The assistant is disabled until the server has an `OPENROUTER_API_KEY`.

Principles (all enforced in code and covered by tests):

* **The compiler is the source of truth.** Diagnostics are never taken from the browser or from the model: the server
  re-runs `yomnova check` on the submitted code and passes that output verbatim.
* **Suggest only.** The assistant has no way to write to the editor. Suggestions are shown, can be copied, diffed and
  *optionally* validated on a copy; the editor changes only when you type.
* **No AI-generated code is executed.** Validation runs `yomnova check` (parse, types, ownership), never `run`.
* **Answers are grounded** in this repository's own documentation and examples, retrieved per request.
* **The API key stays on the server.**

## Setup

1. Create a key at <https://openrouter.ai/keys>.
2. Put it in the server environment (see `.env.example`; never commit a real key):
   ```bash
   OPENROUTER_API_KEY=sk-or-...  npm run dev
   ```
3. Build the compiler (`npm run build:compiler`), open the Playground, press **AI Assistant**.

Without a key the panel says the assistant is off; no model request is possible.

### Free models, chosen by the server

Visitors never see or choose a model. The server tries three built-in free OpenRouter models in order and falls back to
the next one when a request fails: `google/gemma-4-26b-a4b-it:free`, `cohere/north-mini-code:free`,
`nvidia/nemotron-3-ultra-550b-a55b:free` (listed as free on 2026-11-05). Override with `AI_FREE_MODELS` (comma-separated,
max 6). They have **not been called live from this project** (the build sandbox cannot reach OpenRouter); free ids change
often, and an id OpenRouter no longer lists is simply skipped. Responses and the UI show only "YOMNOVA AI", never a model
id, documentation id or file name.

### Short answers

*Explain error* returns one short sentence on what is wrong, at most one on why, and exactly one fix (before/after). Rule
titles, source lists and caveat lists are removed on the server, file names in model text are stripped, and sentence and
length caps are enforced by the server, not only requested in the prompt.

### Environment variables

| Variable | Default | Meaning |
|---|---|---|
| `OPENROUTER_API_KEY` | — | Enables the assistant. Server-side only. |
| `AI_FREE_MODELS` | the three above | Models the server may use, in preference order. |
| `AI_MODEL_ALLOWLIST` | the pool | Optional extra filter on the provider's list (ids or prefixes). Normally unset. |
| `AI_MAX_OUTPUT_TOKENS` | 1200 | Output cap per request (200–4000, also capped by the model's own limit). |
| `AI_TIMEOUT_MS` | 60000 | Provider timeout (covers the whole stream). |
| `AI_RATE_PER_MINUTE` / `AI_RATE_PER_HOUR` | 20 / 300 | Per-client-address limit, and a global hourly cap. |
| `AI_TRUST_PROXY` | 0 | Set to `1` to key rate limits on `X-Forwarded-For` (only behind a trusted proxy). |
| `AI_MODEL_CACHE_MS` | 600000 | How long the model list is cached. |
| `OPENROUTER_BASE_URL` | `https://openrouter.ai/api/v1` | Override for tests or a gateway. |
| `OPENROUTER_SITE_URL`, `OPENROUTER_APP_TITLE` | — / `YOMNOVA Playground` | Optional app attribution headers sent to OpenRouter. |

## Using it

| Action | Where | What is sent |
|---|---|---|
| **Explain with YOMNOVA AI** | Diagnostics tab after a failed run | The editor code; the server derives the diagnostic itself |
| **Suggest a fix** | Diagnostics tab | Editor code (+ selection and your typed question, if any) |
| **Explain selection** | Playground toolbar (select code first) | Only the selected text |
| **Ask / Syntax tutor** | Panel input | Your message (+ editor code only if you tick *editor code*) |
| **Debug** | Panel mode | Editor code and the real compiler output |

The panel shows what is attached to each message. Switching model keeps the conversation and posts a note; if a saved
model disappears from OpenRouter you are told and nothing is chosen for you.

Suggestion cards offer **View suggestion**, **Review diff**, **Copy**, and **Validate with compiler**. Validation
applies the suggestion to a *copy* of your code (only when its `before` text matches exactly once), runs `yomnova check`
and shows the real result. Unvalidated suggestions are labelled *Not compiler-validated*. The "confidence" label is the
model's own rating, not a probability.

## Architecture

```
Playground.tsx ── Assistant.tsx ── src/lib/ai.ts ──HTTP──► server.ts ─► server/ai/routes.ts
                                                                         │  ├─ knowledge.ts   (docs + examples → chunks → retrieval)
                                                                         │  ├─ diagnostics.ts (CLI text → structured diagnostics)
                                                                         │  ├─ prompts.ts     (versioned system prompts, data fencing)
                                                                         │  ├─ openrouter.ts  (model list/cache, requests, SSE, errors)
                                                                         │  ├─ schema.ts      (response validation)
                                                                         │  └─ ratelimit.ts
                                                                         └─ the same `yomnova` execution path as the Playground
```

Reused from the existing project: the `node:http` server, its temp-directory compiler execution with timeout, output
cap and concurrency limit, the JSON body reader, and the Playground UI. No new runtime dependencies were added.

### API (all under `/api/ai/`)

| Route | Purpose |
|---|---|
| `GET /models` | `{configured, defaultModel, models[]}` from OpenRouter (cached). With no key: `configured:false`. |
| `POST /chat` | Streams (`text/event-stream`: `meta`, `delta`, `done`, `error`). Modes `chat`, `tutor`, `explain_code`, `debug`. |
| `POST /explain-diagnostic` | `{model, code, diagnosticIndex?, messages?}` → validated `error_explanation` + the real diagnostics. |
| `POST /suggest-fix` | `{model, code, question?, selection?}` → validated `fix_suggestions`. `compilerValidated` is always `false`. |
| `POST /validate-suggestion` | `{code}` → `yomnova check` result on that exact program. Never executes it. |

Not implemented: a feedback endpoint (there is no storage for it) and conversation persistence (the conversation lives
in the browser tab only; **New** clears it). There is also no user authentication in YOMNOVA, so limits are per client
address; put the server behind your own authentication before exposing it publicly.

### Diagnostics

The compiler prints text (`file:line:col: severity: message`, an excerpt, an optional `= hint:`). It has no error codes
and does not name the stage that failed, so the adapter exposes neither as fact: `stage` is a best-effort label
inferred from the message wording and may be `null`. The raw compiler text is always preserved and shown.
`explain-diagnostic` runs `check`; only if that passes does it run the program once (the Playground's own sandboxed
`run`) to obtain a runtime error.

### Knowledge retrieval

At start-up the server splits `docs/LANGUAGE_SPECIFICATION.md`, `GRAMMAR.md`, `ARCHITECTURE.md`, `ROADMAP.md`,
`README.md` into heading-delimited chunks and adds every program in `yomnova/examples/` (the intentionally invalid ones
are labelled as such). Each request is scored with BM25-style keyword weighting; the specification and grammar are
boosted over README/roadmap text, and the diagnostic's inferred stage adds topic terms. At most 4 chunks / 6000
characters are sent. **Update the docs and the assistant's knowledge updates with them**; there is no separate index to
rebuild. Source ids (`file#heading`) are shown to the user, and any source id the model cites that was not supplied is
removed and reported.

### Prompt-injection and output handling

* Code, selections, compiler output and doc excerpts are wrapped in `<untrusted-*>` / `<knowledge>` blocks and the system
  prompt says they are data; delimiter look-alikes inside them are neutralised.
* Structured answers use OpenRouter JSON-schema outputs where the model advertises them, JSON mode otherwise, and a
  tolerant parser as a last resort. Every answer is validated (types, lengths, control characters stripped, unknown
  sources removed); invalid answers return `502` instead of being shown.
* The UI renders model text as plain React text. There is no `dangerouslySetInnerHTML`, no Markdown-to-HTML, and nothing
  from a model is executed.
* Logs contain route, model, status, duration and token counts only: never prompts, code, headers or keys.

## Privacy

Your prompts, selected code, editor code (when attached) and compiler output are sent from the server to OpenRouter and
on to the model provider you select. OpenRouter documents that retention and training policies differ **by provider**
and that account privacy settings control whether requests may be routed to providers that may train on data
([OpenRouter: privacy and logging](https://openrouter.ai/docs/features/privacy-and-logging), checked 2026-11-02). This
project makes no zero-retention or confidentiality promise: review your OpenRouter privacy settings and the chosen
provider's terms before sending code you do not want shared.

## Testing

```bash
npm run test:ai                               # backend: 46 tests, mocked OpenRouter, REAL compiler (set YOMNOVA_BIN)
npm run build && node tests/e2e/ai-assistant.e2e.mjs   # browser end-to-end (needs Playwright; mock provider)
```

Backend tests cover the model registry and caching, model validation, request construction, missing key, provider
errors, timeouts, cancellation, malformed and empty responses, streaming, structured-output validation, prompt
construction and fencing, retrieval, rate limiting, log hygiene, and the full explain → suggest → validate path against
the real compiler. The end-to-end script drives a real browser through Explain Error → suggestion → diff → validate,
selected-code explanation, model switching, cancel, and checks that the editor text never changes and the key is
absent from the page and bundle. `tests/e2e/mock-openrouter.mjs` is a test fixture that returns canned, labelled answers.

## Known limitations

* **Live OpenRouter behaviour has not been verified**; all automated tests use a mock provider.
* Frontend behaviour is covered by the Playwright script (Chromium only); there are no component-level unit tests.
* "Streaming" is requested for all chat modes and relies on OpenRouter normalising it; OpenRouter does not publish a
  per-model streaming flag. Structured modes (explain / suggest) are not streamed because the JSON must be validated whole.
* Answer quality depends on the model. Retrieval is keyword-based and can miss relevant text; the assistant is told to
  say when it cannot verify something, but a model may still be wrong. Only the compiler's diagnostics are authoritative.
* Validation can only locate a suggestion when its `before` text matches the editor exactly once.
* No authentication, per-user quotas, persistence or feedback collection.
