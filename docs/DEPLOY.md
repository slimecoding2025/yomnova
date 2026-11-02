# Deploying YOMNOVA (Playground + site)

The web app needs a host that can run **Node 20+ and the compiled `yomnova` Linux binary in one long-running
process**. Static-only or serverless hosts do not fit as-is. The repository ships a `Dockerfile` (compiler build →
web build → small runtime image) and a Render Blueprint (`render.yaml`).

> **Status:** the Dockerfile's file layout was verified by assembling the same files locally and running the server
> (`/api/version`, `/api/examples`, `/`, `/api/check` all answered). The image itself has **not** been built and the
> Render deployment has **not** been tried: the authoring environment had no running Docker daemon or registry access.
> Expect to read the first build log.

## Render, free plan ($0)

1. Push this repository to GitHub (already done).
2. Sign in at <https://render.com> with GitHub.
3. **New → Blueprint**, pick the repository. Render reads `render.yaml` (Docker web service, plan `free`).
   Or **New → Web Service → Docker**, choose the *Free* instance type and `/api/version` as the health-check path.
4. Wait for the build. The first build compiles the Rust workspace and bundles the site, so it takes several minutes.
5. Open the service URL, scroll to the Playground and press **Run**.

What to expect on the free plan (from [Render's free-instances page](https://render.com/docs/free), checked 2026-11-02):

* The service **spins down after 15 minutes without traffic** and takes about a minute to wake on the next request.
* 750 free instance hours per month per workspace; when they run out, free services are suspended until next month.
* The filesystem is ephemeral. YOMNOVA stores nothing, so this does not matter.
* Whether a payment method is needed was not stated on that page; Render may ask for one. Free-tier terms change:
  re-read the page before relying on it.

## Before the site is public

* **Do not set `OPENROUTER_API_KEY` on a public instance** unless something in front of it authenticates users: the AI
  routes only have per-address rate limits, and every request spends your OpenRouter credit.
* The Playground executes visitors' programs (5 s limit, 1 MiB output, 4 concurrent jobs, no file or network access in
  the language) but the server has no OS sandbox; the container is the boundary. Expect abuse on any public host.
* `index.html` references the social image with a relative URL. Once you know the public address, make `og:image`
  an absolute `https://…/brand/…` URL so link previews work.

## Other hosts

Anything that runs a Docker image works with the same `Dockerfile` (`docker build -t yomnova .` then
`docker run -p 3000:3000 yomnova`; the server reads `PORT`). Hosts with a free tier change often; compare their current
terms rather than this document.

## Without Docker

```bash
npm run build:compiler && npm install && npm run build && npm start     # serves dist/ and the API on $PORT (default 3000)
```
