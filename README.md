# PQC Security Inspection Course

A rebuild of the lab's three post-quantum cryptography teaching tools as one containerised
system: a Next.js frontend, a Rust API and worker, Postgres, Redis, and a set of prober
containers — each wrapping a different TLS stack.

| Module | What it does | Route |
|---|---|---|
| **PQ-CAS** | Runs a *group × signature-algorithm* matrix against a target and shows the handshake back, message by message | `/tests` |
| **PQC KEM DEMO** | Library compatibility matrix, `key_share` group handshakes, and a black-box KEM preference-order probe | `/kem/{library,group,priority}` |
| **CAVP station** | The five-step ACVP flow for ML-DSA: registration → question pack → local compute → results → report | `/cavp/{key_gen,sig_gen,sig_ver}` |

Everything is bilingual (English / 繁體中文) via `next-intl`; the locale is the first path
segment.

## Quick start

```sh
cp deploy/.env.example deploy/.env      # adjust the password at least
make dev                                # build + start the stack
open http://localhost:8088/en
```

`make dev` starts Postgres, Redis, the API, the worker, the OpenSSL and Go prober adapters,
the web app, Caddy, and two lab targets. First build takes a few minutes (Rust release
build); afterwards it is seconds.

```sh
make dev-full   # adds the BoringSSL and wolfSSL adapters (built from source, slow)
make test       # cargo + vitest + go test
make lint       # clippy, eslint, gofmt, rustfmt
make e2e        # Playwright against the running stack
make down       # stop
```

## Architecture

```
browser ──► Caddy ──┬─► /api/*  ──► api (axum)  ──► Postgres
                    │                   │
                    └─► /*      ──► web  └────────► Redis ──► worker ──► adapter-{openssl,boringssl,wolfssl,go}
                        (Next.js)                                              │
                                                                        TLS handshake
                                                                               ▼
                                                                    target (tls-good / tls-vuln / anything)
```

* **`apps/web`** — Next.js 16 App Router. Server Components read through the API over the
  internal network; the browser reaches the same API same-origin through Caddy, which
  matters for the SSE streams that drive the live matrix and the KEM console.
* **`services/api`** — axum. Creates tasks, expands a task into one subtask per (group, sig
  alg) pair, queues them on a Redis stream, and serves matrices, reports and CAVP packs.
* **`services/worker`** — consumes the stream (consumer group + `XAUTOCLAIM`, so a crashed
  worker's jobs get picked up), calls an adapter, classifies the result, persists the
  report, and publishes progress for SSE.
* **`adapters/*`** — one container per TLS stack, all speaking the same contract
  (`crates/adapter-proto`): `GET /capabilities`, `POST /probe`. The Rust adapter drives
  `openssl s_client` / `bssl client` / the wolfSSL example client; the Go adapter uses
  `crypto/tls` directly.
* **`crates/cavp-validate`** — grades submitted ACVP `results.json` against the answer keys
  in `deploy/seeds/cavp_packs.json`.

### Why adapters are separate containers

The compatibility matrix is a claim about *TLS stacks*, so each column has to be that stack,
not a re-implementation of it. Keeping them as long-lived HTTP services (rather than
spawning a container per probe) avoids handing the worker a Docker socket and keeps a probe
in the millisecond range.

A cell is `unsupported` when the adapter's `/capabilities` never advertised the algorithm,
and `disabled` (未啟用) when the deployment does not run that adapter at all — neither is
reported as a failure of the target.

## Data

* `algorithms` — the seeded catalog behind every checkbox grid and matrix axis. Entries with
  `enabled = false` render greyed out.
* `endpoints` — lab targets (`A server`, `B server`, `正常 server`, `漏洞 server`).
* `tests` / `subtasks` / `subtask_reports` — a task, its matrix cells, and the heavy report
  blobs (split out so painting the matrix stays cheap).
* `kem_runs` / `kem_run_logs` / `kem_matrix_cells` — KEM DEMO runs and their console.
* `cavp_packs` / `cavp_sessions` — question packs (prompt + answer key) and submissions.

Seeds live in `deploy/seeds/` and are applied by `pqcas-api seed`, which is idempotent and
runs on every bring-up.

### Regenerating the CAVP answer keys

```sh
make vectors
```

`tools/gen-cavp-vectors.mjs` derives the packs from fixed seeds using the same
`@noble/post-quantum` build the browser runs in step 03, so "the reference answer" and "what
a correct student computes" are the same computation. CI re-runs the generator and fails if
the checked-in seeds differ.

## Lab targets

* **`tls-good`** — OpenSSL 3.5 serving *two* certificates (ECDSA P-256 and ML-DSA-65) so both
  classical and post-quantum signature probes have something to negotiate. `:11000` is the
  A/正常 server, `:11001` the B server used by the priority probe.
* **`tls-vuln`** — the 漏洞 server: TLS 1.2 only, classical groups only, ECDSA certificate.
  Classical probes succeed and every post-quantum probe fails, which is the lesson.

## CI/CD

| Workflow | Trigger | What it does |
|---|---|---|
| `ci.yml` | PR, push to main | rustfmt + clippy + `cargo test`; `go vet`/`go test`/gofmt; eslint, tsc, vitest, `next build`; regenerates the CAVP seeds and fails on drift; brings the stack up with compose and runs Playwright |
| `images.yml` | push to main, tags | Builds each target from `docker/Dockerfile`, pushes to GHCR with provenance + SBOM, and gates on a Trivy scan of the pushed digest |
| `deploy.yml` | after `images.yml` | Runs on a **self-hosted runner on the lab host** (the box is on a private LAN): pulls the pinned tag, runs migrations and seeds, rolls the stack forward, waits on `/readyz`, smoke-tests a real probe, and rolls back on failure |

The deploy job needs a runner registered on the lab host with the labels `self-hosted` and
`pqc-lab`, and a GitHub environment named `lab`.

## Configuration

Everything is environment-driven; see `deploy/.env.example`. The ones worth knowing:

| Variable | Purpose |
|---|---|
| `ADAPTERS` | `name=url` pairs the worker may dispatch to. Anything absent renders as 未啟用. |
| `WORKER_CONCURRENCY` | Simultaneous probes. Also a politeness limit towards the target. |
| `MAX_SUBTASKS_PER_TEST` | Upper bound on one task's matrix (default 512). |
| `PROBE_TIMEOUT_MS` | Per-handshake timeout. |

## Repository layout

```
apps/web/            Next.js app (routes, components, i18n messages, e2e)
services/api/        axum API + SQL migrations
services/worker/     job consumer, probe dispatch, KEM run modes
adapters/cli/        Rust adapter for OpenSSL / BoringSSL / wolfSSL (+ a mock backend)
adapters/go/         Go adapter using crypto/tls
crates/              shared domain types, adapter contract, CAVP validation
deploy/              compose files, Caddy, seeds, lab targets
docker/Dockerfile    every image, one build target each
tools/               seed generators
```
