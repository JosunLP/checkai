<!--
Thank you for contributing to CheckAI!
Please fill in the sections below and tick every checklist item that applies.
See .github/CONTRIBUTING.md for the full contribution guide.
-->

## Summary

<!-- What does this PR change, and why? Keep it short and factual. -->

## Related issues

<!-- Link issues this PR closes or relates to, e.g. "Closes #123". -->

## Type of change

<!-- Tick all that apply. -->

- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (fix or feature that changes existing behavior or APIs)
- [ ] Documentation only
- [ ] CI / tooling / dependencies

## Affected areas

<!-- Tick all that apply. -->

- [ ] Rust core (`src/` — engine, CLI, REST API, WebSocket, analysis, storage)
- [ ] Web UI (`web/`)
- [ ] Desktop app (`desktop/`)
- [ ] WASM crate / npm package (`wasm/`, `npm/`)
- [ ] Documentation (`docs/`, `README.md`)
- [ ] Locales (`locales/`)

## Quality gates

<!-- All four commands must pass locally before requesting a review.
     CI runs them with RUSTFLAGS=-Dwarnings, so warnings fail the build. -->

- [ ] `cargo fmt --all -- --check` passes
- [ ] `RUSTFLAGS=-Dwarnings cargo clippy --all-targets --all-features` passes with no warnings
- [ ] `cargo test --all-features` passes
- [ ] Frontend checks pass if `web/` or `desktop/` changed (`bun run check` in the affected directory)

## Internationalization

<!-- CheckAI ships 8 locales: en, de, es, fr, ja, pt, ru, zh-CN. -->

- [ ] All user-facing strings go through `rust-i18n` (`t!("key", ...)`) — no hard-coded English in code paths users see
- [ ] Every new or changed i18n key exists in **all 8** locale files (`locales/en.yml`, `de.yml`, `es.yml`, `fr.yml`, `ja.yml`, `pt.yml`, `ru.yml`, `zh-CN.yml`)
- [ ] No i18n changes in this PR

## Documentation

- [ ] Public Rust items have rustdoc comments (`///`, `//!` for modules)
- [ ] `README.md`, `docs/`, and `docs/AGENT.md` are updated if protocol, API, or user-visible behavior changed
- [ ] `CHANGELOG.md` has an entry under the appropriate heading (for user-visible changes)
- [ ] No documentation changes needed

## Additional notes

<!-- Screenshots, benchmark numbers, migration notes, open questions for reviewers. -->
