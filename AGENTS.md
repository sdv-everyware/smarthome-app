# smarthome_app: instructions for coding agents

A harry (HAR, display_safety) app: the smart home panel on SDV Media, on the reference board
(Qualcomm RB3, HDMI 1920x1080). In the tree this directory is
`packages/services/display_safety/utils/smarthome-app`.

The project's guide (the repositories, every skill, the rules for every change) is
`vendor/sdv-everyware/everyware/AGENTS.md`: read it first.

Before writing, building or running a harry app, read:

1. ui-lib's skills (`vendor/sdv-everyware/ui-lib/skills/`: `new-app`, `use-component`,
   `sheet`, `ui-component`): how an app on the Library is made, from the Figma layer to the
   screen, build and run on the board.
2. `docs/harry-app-anatomy.md`: how this app's code works, step by step, with file:line links.

Short version of the rules:

- Show the concept and wait for agreement before writing code.
- On the board or VM: say what each command does and whether it changes state; runtime changes
  only, never the image unless asked.
- Changes to harry itself (`packages/services/display_safety/framework`) only with the owner's OK.
- No AI attribution in commits; never push without asking.

The design is in this repository: `design/` (the panel's patches, `build.sh`, the designer's export
in `source/`, the app's own weather pictures); `design/build.sh --dcf` builds
`design/build/smarthome.fig` and `data/smarthome.dcf`. After a design build give the user the full
path of the built `.fig`. The skill for it: `vendor/sdv-everyware/everyware/skills/figma-design`.

## The rules of an app

- **Library components only on the screen.** A new kind of control is a new library component
  (the `ui-component` skill): stop and ask the user, do not draw it in the patch.
- **What the app shows that is its own is the app's:** its pictures are a component set of its
  design, its type in the code implements `Component` with `ComponentKind::App` and `Slotted`.
  Nothing of an app goes into the UI library, also not under a generic name.
- **A bundle sends information; how it looks is decided here** (the presenter and the modules it
  uses), not in a bundle and not in the library.
- **The design's names are a contract** with `src/ui_model.rs`: change both in one commit.
- **Nothing hidden in the design** (a hidden layer never reaches the device): hide with
  `slot_shown`, `grid_shown` or an opacity.
- **Rules that need no screen** (a game, a calculation) are a module of their own with host
  tests.

A change lands through its own branch, a review by another agent and the user's word
(`vendor/sdv-everyware/everyware/AGENTS.md`, "How a change lands"; `scripts/feature.sh` there).
The hooks in `scripts/git-hooks/` refuse a commit on `main`, by another author, with something
private in it or with an AI attribution (`vendor/sdv-everyware/everyware/scripts/setup-agents.sh`
installs them).
