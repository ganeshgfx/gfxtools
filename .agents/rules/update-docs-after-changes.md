# MANDATORY: Keep SKILL.md and README.md in sync with ALL code changes

> **This rule is NON-NEGOTIABLE. Violations are unacceptable. Every code change MUST include doc updates before the task is marked complete.**

## Trigger

ANY modification to the codebase that touches:

- Source modules (`src/*.rs`) — add, remove, rename, or change responsibility
- CLI commands (`cli.rs` `Command` enum)
- Dependencies (`Cargo.toml`)
- Context menu entries (`context_menu.rs`)
- Config fields (`config.rs`)
- GUI windows (Win32 or eframe/egui)
- Install scripts (`install.ps1`, `install-plugin.ps1`)
- Test files (`tests/`)
- Build configuration (`build.rs`, `resources.rc`, `.cargo/config.toml`)
- Architecture, entry flow, or conventions
- Error variants (`error.rs`)
- Plugin files (`plugin/`)

## Required Actions

**BOTH files MUST be updated in the SAME task/conversation — never defer to "later":**

1. **`.agents/skills/video-yoinker-project/SKILL.md`**
   - Module reference table
   - Architecture / flow sections
   - Dependency list
   - Conventions & patterns
   - Common tasks
   - Filesystem locations
   - Any other affected section

2. **`README.md`**
   - Features list
   - Architecture diagram
   - CLI usage
   - Project structure tree
   - Source module details
   - Dependency table
   - Any other affected section

## Enforcement Checklist

Before responding that a task is complete, verify:

- [ ] Did I add/remove/rename any module? → Update module table in SKILL.md + structure tree and module details in README.md
- [ ] Did I add/remove a CLI command? → Update SKILL.md architecture + README.md CLI usage
- [ ] Did I add/remove a dependency? → Update both dependency lists
- [ ] Did I add/remove a config field? → Update SKILL.md common tasks + README.md configuration section
- [ ] Did I change any GUI? → Update SKILL.md module reference + README.md module details
- [ ] Did I change context menus? → Update SKILL.md filesystem locations + README.md features
- [ ] Did I change conventions/patterns? → Update SKILL.md conventions section

If ANY checkbox applies and the corresponding doc was NOT updated, **the task is NOT complete — go update the docs NOW.**

## Rules

- Only update sections relevant to the change — don't rewrite the entire file for a small addition.
- Keep both files consistent with each other.
- Read the SKILL.md first to understand current state before making updates.
- If unsure whether a change warrants a doc update, UPDATE ANYWAY — false positives are acceptable, stale docs are not.
