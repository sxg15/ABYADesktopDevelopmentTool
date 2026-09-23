# Gameplay Architecture Contract

Use this contract after feasibility approval and before gameplay mutation.

## Target Ownership

| Responsibility | Preferred target |
|---|---|
| Match phase, timer, spawn schedule, victory, loss, reset | Global or Level coordinator |
| Input routing | Input binding or interactive CustomUI event |
| Movement, weapon, health, death, respawn | The entity Prefab or exact object event |
| Projectile movement and impact | Native projectile behavior and collision/trigger events |
| UI, camera, animation, audio, VFX | Client event on the presenting object or CustomUI node |

A complete ABYA-LUA String is one behavior unit, not the whole game. Scripts
bound to different events are intentionally self-contained; share durable state
through the documented ABYA data APIs when coordination is required.

Do not implement an entity manager that queries every object and writes their
positions, collision state, damage, and presentation in one per-frame Global or
Level loop. Do not use distance polling when a permitted collision, projectile,
or trigger event expresses the same behavior.

## Required Workflow

1. Call `editor_gameplay_architecture_snapshot` after the target archive and
   level are ready.
2. Create `artifacts/gameplay-architecture.v1.json` from the bundled template.
   Use only exact targets and event IDs returned by Runtime MCP.
3. Call `editor_gameplay_architecture_validate` with the complete JSON object.
   Errors block implementation. Warnings require correction or an exception
   containing `ruleCode`, exact `targetKey`, reason, and evidence.
4. Scaffold and validate each Lua behavior against its declared target. Shared
   FreeObject behavior belongs on `{kind:"prefab",prefabId:"..."}` and is
   written with `editor_prefab_logic_binding_*`.
5. After implementation, call `editor_gameplay_architecture_lint` with the same
   manifest. Re-run runtime tests after every structural correction.

Prefab writes synchronize all linked instances and therefore require explicit
high-impact approval. Read the binding immediately before writing and pass its
`prefabRevision` as `expectedRevision`.

## Example Decomposition

For a tank game, Level logic owns match phases and victory. A tank Prefab owns
movement, firing cooldown, health, death, and respawn events. A projectile
Prefab or native projectile system owns travel and impact. Client-side tank or
UI events own animation, camera, audio, VFX, HUD, and result presentation.
