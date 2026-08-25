# ORCHESTRATOR

`vortex-bevy`'s implementation is driven by a multi-agent orchestration loop.
This document describes the workflow, the agents, the state machine, and the
invariants. It is the canonical entry point for anyone — human or LLM —
joining mid-stream.

> **TL;DR.** `wiggum generate <your-plan>.toml` materialises the plan into
> `IMPLEMENTATION_PLAN.md`, `PROGRESS.md`, `tasks/`, and `features.json`. The
> orchestrator agent reads `PROGRESS.md`, dispatches work to subagents,
> independently verifies each task, and loops until T01–T32 are all `[x]`.

## The plan, the tasks, the progress

| File | Role |
|---|---|
| `<your-plan>.toml` | Single source of truth — phases, tasks, hints, criteria, evaluator weights, completion standard. Edit this, never `IMPLEMENTATION_PLAN.md` directly. |
| `IMPLEMENTATION_PLAN.md` | Human-readable plan rendered from the TOML by `wiggum generate`. |
| `tasks/T{NN}-{slug}.md` | One file per task. Exit criteria, hints, evaluation rules. Materialised by `wiggum generate`. |
| `PROGRESS.md` | Live state — what is `[ ]`, `[~]`, `[x]`, `[!]`. Plus the **Accumulated Learnings**, **Codebase State**, and **Completion Standard** sections that travel with every subagent dispatch. |
| `features.json` | Per-task criteria registry. Evaluator writes `"passes": true` here on PASS. |
| `.opencode/agents/*.md` | Agent prompts (this project uses opencode). |
| `ORCHESTRATOR.md` | This file. Human-readable workflow reference. |
| `.vscode/*.prompt.md` | Equivalent prompts for VS Code Copilot agent mode. Both coexist. |

The TOML is the source. The other files are derived state. To change the plan,
edit the TOML and re-run `wiggum generate`.

## Agents

| Agent | Mode | Role |
|---|---|---|
| `orchestrator` | primary | Reads `PROGRESS.md`, finds the next ready task, dispatches an implementation subagent, runs preflight, dispatches the evaluator and background auditor, retries on failure. |
| `general` (built-in) | subagent | The implementation subagent. Spawned via the `task` tool with `subagent_type: "general"` and the inline `<SUBAGENT_PROMPT>` from the orchestrator prompt. |
| `evaluator` | subagent | Independent QA. Runs preflight, weights the rubric, flags hollow passes. Writes `features.json`. |
| `planner` | subagent | Decomposes goals into wiggum tasks. Used for *new* plan sections, not for day-to-day implementation. |
| `background-auditor` | subagent | Post-task cross-cutting audit — wiring, stub creep, dependency surface, API consistency, security surface. |

### Per-agent tooling

| Agent | read | edit | bash | task | webfetch |
|---|---|---|---|---|---|
| `orchestrator` | ✓ | ✓ | ✓ | ✓ | ask |
| `general` (subagent) | ✓ | ✓ | ✓ | — | — |
| `evaluator` | ✓ | ✓ (features.json only) | ✓ (preflight) | — | deny |
| `planner` | ✓ | ✓ | deny | — | deny |
| `background-auditor` | ✓ | deny (notes via orchestrator) | ✓ | — | deny |

The orchestrator is the only agent that writes `PROGRESS.md`. The evaluator
and background-auditor return structured findings; the orchestrator folds them
into the state file.

## Task state machine

```
   ┌──────┐    dispatch    ┌──────┐   preflight OK    ┌──────┐
   │ [ ]  │ ─────────────▶ │ [~]  │ ──── + eval ────▶ │ [x]  │
   │ idle │                │work  │                  │ done │
   └──────┘                └──────┘                  └──────┘
                              │                           ▲
                              │ eval FAIL / preflight FAIL│
                              ▼                           │
                           ┌──────┐    retry ≤ 3         │
                           │ [~]  │ ─────────────────────┘
                           │retry │
                           └──────┘
                              │ retries exhausted
                              ▼
                           ┌──────┐   human restarts
                           │ [!]  │ ─────────────────────▶ loop
                           │stuck │
                           └──────┘
```

| State | Meaning | Owner |
|---|---|---|
| `[ ]` | Not started; may have unfulfilled dependencies | none |
| `[~]` | Claimed by an in-flight subagent | one subagent |
| `[x]` | Done: preflight passed, evaluator PASS, audit clean | none |
| `[!]` | Blocked. Pre-flight or evaluator FAILed after retries; orchestrator paused. | human |

The orchestrator only dispatches a task when all its `depends_on` slugs are `[x]`.

## The implementation loop

For each `[ ]` task whose dependencies are `[x]`:

1. **Gate check.** If the task file begins with `⛔ GATE`, emit the banner verbatim and stop. The human must confirm.
2. **Claim.** Mark `[~]` in `PROGRESS.md`.
3. **Extract context.** Copy the full **Accumulated Learnings**, **Codebase State**, and **Completion Standard** sections out of `PROGRESS.md`.
4. **Dispatch implementation.** Spawn a `general` subagent via `task(subagent_type="general", prompt=<SUBAGENT_PROMPT with injected context>)`. The subagent implements and runs preflight.
5. **Independent preflight.** Orchestrator runs the same preflight itself. If it fails, the subagent's `[x]` claim is rejected — mark `[!]` and retry.
6. **Evaluator.** Spawn `evaluator` subagent. Weighted rubric must score ≥ 8/10; hard-fail mode on.
7. **Background auditor.** Spawn `background-auditor` subagent. Findings are recorded in `PROGRESS.md` notes.
8. **Verify.** Confirm the task is `[x]` in `PROGRESS.md`. If not, mark `[!]` and continue to the next task (failures don't block unrelated tasks).
9. **Commit.** The implementation subagent commits per task. The orchestrator never commits on its own.

After 3 retries with preflight or evaluator failure, the orchestrator pauses
and emits a `⛔ Task T{NN} failed after 3 retries. Human intervention required.`
banner.

## Preflight

The preflight is a single command, called out verbatim in every agent prompt:

```bash
cargo build --workspace --all-targets --all-features && cargo test --workspace --all-features && cargo fmt --all -- --check && cargo l && cargo deny --all-features check && cargo audit
```

The full command lives in `[preflight]` of the plan TOML. **Never loosen any flag
to make a task pass — fix the code.**

## Evaluator rubric

This project uses a **weighted rubric**. Score each criterion from 0–10, then compute:

| Criterion | Weight | What it proves |
|---|---|---|
| `tests-and-fixtures` | 25% | All workspace tests pass; fixture-based tests cover success, malformed input, unsupported features, and failure reporting for the task’s boundary. |
| `strict-quality` | 25% | Formatting, the complete Nick Clippy profile, cargo-deny, and cargo-audit pass with no warnings or unjustified suppressions. |
| `architecture-and-error-boundaries` | 20% | Pure domain boundaries hold, ports remain narrow and consumer-owned, raw strings do not leak past adapters, and production code has no unwrap/expect/panic/index slicing. |
| `determinism-and-traceability` | 15% | Output is deterministic and idempotent; manifests preserve typed source identity, conversion profile, classification, diagnostics, and provenance. |
| `runtime-and-documentation` | 15% | The feature is wired through the CLI or Bevy application, public APIs have rustdoc/examples where appropriate, and user-facing documentation matches actual behavior. |


A weighted score ≥ 8 with **no criterion FAIL** is a PASS. Hard-fail mode is on — any single criterion failure is a FAIL regardless of score.

## Completion Standard

A standing directive on the bar for shipped work, owned by the project owner.
Edited rarely — typically once when the project is bootstrapped. The
orchestrator copies this section verbatim into every subagent dispatch so
that every fresh agent (including ones spun up after compaction) receives
the same load-bearing bar:

```
Every task ships only when the root fix is complete, not when a narrow happy path
compiles. Preflight passes with zero errors and zero warnings; all stated
must-haves and evaluation criteria are demonstrably true; success, malformed,
unsupported, and failure paths are tested; user-facing docs and fixture notes
are updated; generated artifacts are deterministic; and no production stubs,
todo!(), unimplemented!(), placeholder defaults, #[allow(...)] suppressions,
unwrap(), expect(), panic!(), or unchecked indexing remain. The task must leave
the workspace more coherent than it found it and update PROGRESS.md with verified
codebase state and durable learnings.

```

## Accumulated Learnings and Codebase State

Two more persistent sections in `PROGRESS.md` travel with every subagent dispatch:

- **Accumulated Learnings** — bullet list of discoveries appended after each
  task. Format: `- T{NN}: {what you learned}`. Cross-task traps, dead-ends,
  non-obvious invariants. Read by the orchestrator before every dispatch so
  mistakes are not repeated.
- **Codebase State** — concise summary of what currently exists in the
  workspace after the last task. Updated after each `[x]`. A fresh agent
  should be able to orient from this section alone.

The orchestrator injects all three sections (Accumulated Learnings, Codebase
State, Completion Standard) verbatim into the implementation subagent's
`prompt` parameter on every dispatch.

## Gates

A task file may begin with a `⛔ GATE` banner. When the orchestrator encounters
one, it:

1. Emits the banner verbatim.
2. **Stops** — no further dispatch.
3. Waits for the human to restart the orchestrator.

Use gates sparingly: external service credentials, irreversible deployments,
schema migrations, anything where a wrong autonomous action costs more than a
human checkpoint.

## Session boundaries

When a context window ends mid-task (compaction or interrupt), the
orchestrator writes a `## Session handoff` section at the bottom of
`PROGRESS.md` with:

- Current task slug and status (`[~]`)
- Files modified so far
- Next concrete action needed

The orchestrator never marks a task `[x]` on the strength of a handoff alone —
on resume, the next iteration must independently verify preflight and the
evaluator must run.

## Running it

### opencode

```bash
# Pick the orchestrator agent in the opencode TUI, or invoke it directly:
opencode --agent orchestrator

# Or via slash command (auto-generated from agent name):
/orchestrator
```

The orchestrator auto-uses `.opencode/package.json` to install the
`@opencode-ai/plugin` runtime when needed.

### VS Code Copilot agent mode

Open `.vscode/orchestrator.prompt.md` in agent mode. The VS Code prompt set is
the dual of the opencode agent set; both implement the same workflow.

### From the CLI (no agent)

```bash
# Validate the plan (substitute your plan's filename)
wiggum validate <your-plan>.toml

# Regenerate PROGRESS.md, IMPLEMENTATION_PLAN.md, tasks/, and the agent prompts
wiggum generate <your-plan>.toml
```

## Failure modes and recovery

| Symptom | Likely cause | Fix |
|---|---|---|
| Preflight fails on the lint command after a `[x]` claim | Implementation subagent added a lint suppression or loosened the profile | Reject the claim; mark `[!]`; retry with explicit instruction not to loosen. |
| Evaluator returns hollow-pass FAIL | `todo!()` / `unimplemented!()` left behind; tests assert nothing meaningful | Mark `[!]`; orchestrator captures finding; next iteration must remove stubs. |
| `PROGRESS.md` corrupted mid-loop | Concurrent edits from two sessions | Stop one session; reconstruct `PROGRESS.md` from `git log -- tasks/` and the latest commit messages. |
| Task stuck at `[~]` after compaction | Subagent session ended without writing `[x]` | Handoff section captures state; resume dispatches a fresh subagent with full context. |
| Plan and `PROGRESS.md` drift out of sync | Manual edits to task files bypassing the TOML | Re-run `wiggum generate`; let the orchestrator re-derive state. |

## See also

- `AGENTS.md` — project rules, lint profile, workspace layout.
- `<your-plan>.toml` — the plan source of truth.
- `.opencode/agents/` — agent prompts (opencode).
- `.vscode/*.prompt.md` — equivalent prompts (VS Code Copilot).
- `IMPLEMENTATION_PLAN.md` — human-readable plan derived from the TOML.

_Generated by [wiggum](https://github.com/greysquirr3l/wiggum)._
