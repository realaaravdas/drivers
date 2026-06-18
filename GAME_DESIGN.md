# Rust Racer — Complete Game Reference

A comprehensive document for LLMs and agents iterating on this codebase. Written from a full read of every source file.

---

## Tech Stack

- **Bevy 0.18.1** — ECS game engine, rendering, UI, asset loading
- **bevy_rapier3d 0.34.0** — 3D rigid-body physics (Rapier under the hood)
- **bevy_camera** — multi-camera viewport utilities
- **rand 0.10.1** — RNG for procedural generation

Build: `cargo run` (dev), `cargo build --release` (optimized). Dev builds have `opt-level = 3` on all dependencies — this is intentional and must not be removed; physics runs unacceptably slow without it.

---

## Game State Machine

```
Splash (3s) → MainMenu → Loading (0.5s) → GeneratingLevel → Racing → Scoreboard
                 ↑                                                         |
                 └──────────────── Main Menu btn ──────────────────────────┘
                 ↑
                 └──────────────── ESC during Racing ──────────────────────┘
```

States are a Bevy `States` enum in `src/game_state.rs`. `PostRace` exists as a state but currently has no behavior — the "Continue" scoreboard button routes to it and it immediately becomes a dead end (no system transitions out of it). This is an open bug / unfinished feature.

### Loading timing rationale
The 0.5 s `LoadingTimer` before `GeneratingLevel` exists because level generation (`generate_level`) is synchronous and blocks the main thread. The delay gives Bevy one frame to render the loading screen before the block occurs.

---

## Resources and Shared Data

### `GameDifficulty` (persists across races)
Stored as a Bevy `Resource`. Set in the main menu, read in `vehicle.rs`, `ai.rs`.

| Field | Default | Menu Range | Step | Effect |
|---|---|---|---|---|
| `ai_aggressiveness` | 1.0 | 0.2 – 3.0 | 0.2 | Scales AI speed/acceleration; controls blocking trigger distance and intensity |
| `steering_sensitivity` | 3.0 | 0.5 – 10.0 | 0.5 | How fast the steering wheel turns and returns to center |
| `top_speed` | 120.0 | 40 – 300 | 10 | Player `max_speed` cap (not currently enforced in physics — see Known Issues) |
| `acceleration` | 500.0 | 50 – 2000 | 50 | Engine force multiplier |
| `laps` | 3 | 1 – 10 | 1 | Total laps for the race |

### `LevelData` (generated per race, cleared on cleanup)
Contains `waypoints: Vec<Vec3>` (ordered circuit waypoints) and `start_pos: Vec3`.

---

## Level Generation (`src/level_gen.rs`)

Everything runs in `generate_level`, which is called `OnEnter(GameState::GeneratingLevel)` and **transitions to `Racing` itself** when done (`state.set(GameState::Racing)`).

### Terrain Formula
```rust
pub fn get_terrain_height(x: f32, z: f32) -> f32 {
    let raw_h = (x / 400.0).sin() * 20.0 + (z / 300.0).cos() * 15.0 - 10.0;
    if raw_h < 0.0 { 0.0 } else { (raw_h * raw_h) / 20.0 }
}
```
This is a pure function — no state. Both the render mesh and physics collider call it independently. **If you change this formula, the physics collider and the vehicle self-righting system both depend on it.** The vehicle uses `get_terrain_height` at runtime every frame to compute ground normal.

### Heightfield Mesh
- 401 × 401 vertices, 8-unit grid spacing → 3200 × 3200 world units total
- Vertex colors encode road/grass: yellow center line (< 1 m from road centerline), gray road (< 15 m), blend zone (15–18 m), dark green grass (> 18 m)
- Material is `Color::WHITE` so vertex colors render unmodified
- Physics collider is a high-resolution `Collider::trimesh` built from the exact same vertex array — no LOD mismatch

### Waypoints
- 24–40 waypoints, arranged as a closed circuit: `angle = i/num_points * 2π`, radius randomly 12–24 (in 40-unit grid units, so actual radius is 480–960 world units)
- `waypoints[0]` is the start/finish line (white gate poles)
- All subsequent waypoints have red gate poles; the next upcoming one is shown orange/yellow via `update_gate_colors`

### Buildings
60×60 grid (±30 in each axis) of `BLOCK_SIZE=40` spacing. Any grid cell that is within `BLOCK_SIZE * 0.8 = 32` units of a track segment is skipped. Others get a random-height (10–50), random-color cuboid with a matching `Collider::cuboid`.

### Waypoint Gates (Ski Gates)
Each waypoint spawns a parent entity with `WaypointMarker(i)` and two `Cylinder` child poles flanking the track direction (±10 units perpendicular to the next-waypoint direction, +4 m vertical). Gate colors are updated each frame by `update_gate_colors` in `vehicle.rs`.

---

## Vehicle Physics (`src/vehicle.rs`)

All vehicles — player and AI — use identical physics components. `Vehicle.is_player` distinguishes which receives keyboard input.

### Components on Each Car
```
RigidBody::Dynamic
Collider::round_cuboid(0.9, 0.4, 1.9, 0.1)   ← slightly smaller than visual 2×1×4
Damping { linear: 0.5, angular: 20.0 }        ← high angular damping prevents spin
Ccd::enabled()                                 ← continuous collision at high speed
ExternalForce                                  ← where we write forces/torques each frame
Velocity                                       ← read for current speed
ReadMassProperties                             ← mass is Rapier default (~1.0)
```

### Force Model (applied every frame in `vehicle_update`)

```
Total Force = engine_force + drag_force + grip_force + downforce
Total Torque = turn_torque + righting_torque
```

| Component | Formula | Notes |
|---|---|---|
| Engine | `forward * throttle * acceleration` | Throttle is ±1.0 |
| Drag | `-forward * fwd_vel * 1.0` | Linear in forward velocity only |
| Grip | `-right * lat_vel * grip_factor` | `grip_factor = 30.0` normal, `8.0` drifting |
| Downforce | `-normal * clamp(fwd_vel_abs * 3.0, 0, 200)` | Only when `height_above_ground < 3.0` |
| Braking | `-forward * fwd_vel * 5.0` | Added to engine_force when Space held |
| Turn torque | `Y * steering * 2000.0 * speed_factor * turn_dir` | `speed_factor` ramps 0→1 over first 5 m/s |
| Righting torque | `tilt_axis * angle * 400.0` | Aligns car up-vector with terrain normal |

**`max_speed` is not enforced.** The `Vehicle.max_speed` field is stored but never read in `vehicle_update`. Speed is implicitly limited by the drag/grip balance — at high speeds drag grows fast enough to create a natural cap, but the actual cap depends on the force balance, not the setting.

### Self-Righting System
Every frame, the car samples `get_terrain_height` at its position plus two neighbor points (+1 in X, +1 in Z) to compute a finite-difference terrain normal. If `height_above_ground < 3.0`, it torques the car toward that normal at strength 400. If airborne (≥ 3.0), it applies extra gravity (`-Y * 400`) and righting toward world-up at strength 200. The spring stiffness (400/200) was deliberately lowered from a much higher value to prevent physics explosions.

### Steering
- Steering angle interpolates toward target at `steering_sensitivity` rad/s; returns at `sensitivity * 1.5`
- Max angle: `1.047 rad` (60°)
- Turning reverses if going backward (`fwd_vel < -0.1`)
- Front wheel visuals (`WheelFrontLeft`, `WheelFrontRight`) are updated every frame to match `steering_angle`

### Controls
| Key | Action |
|---|---|
| W / ↑ | Throttle forward |
| S / ↓ | Throttle reverse |
| Space | Brake (zero throttle, apply -5× fwd_vel force) |
| Shift (L or R) | Drift (drops grip from 30 → 8) |
| A / ← | Steer left |
| D / → | Steer right |
| Escape | Exit to Main Menu |

### Exhaust Smoke
20% chance per frame to spawn a smoke `SmokeParticle` at the `ExhaustPort` child entity. Particles live 0.5–1.5 s, drift in the exhaust direction + random scatter + Y*2 upward, and scale down to zero as they expire. Each particle is a `RaceEntity` and is cleaned up on race exit.

---

## AI System (`src/ai.rs`)

### Spawn Layout
12 AI cars in a staggered 4×3 grid behind the player start:
- Row = `(i + 1) / 2` (1-indexed)
- Column = ±4 units (alternating)
- Z offset = `row * 8.0` behind start

Spec tiers (relative to `GameDifficulty`):
- Cars 1–4: `spec_mod = 1.1` (10% faster)
- Cars 5–8: `spec_mod = 1.0` (equal)
- Cars 9–12: `spec_mod = 0.9` (10% slower)

### AI Navigation (`ai_update`)
Every frame per AI car:

1. **Waypoint advance**: if within 15 m of `next_waypoint`, advance; wrap at end to start new lap
2. **Target computation**: start with the next waypoint position
3. **Blocking behavior** (if player within `40 * ai_aggressiveness` units):
   - Player behind AI: shift target laterally toward player's lane by up to 15 units
   - Player beside AI (< 15 m): lerp target 30% toward player position
4. **Steering**: `target_steering = -right.dot(to_target)` (clamped ±1)
5. **Throttle**: `1.0 * ai_aggressiveness`; reduced to `0.7 * ai_aggressiveness` if `forward_dot(to_target) < 0.5` (sharp turns)
6. **Stuck detection**: if `velocity.length < 2.0` for 2+ seconds, trigger 1.5 s of reversing (throttle = -1, steering reversed)

AI steering lerps: `steering_angle += (target * max_steering - steering_angle) * 0.1` (10% per frame — frame-rate dependent, not multiplied by `dt`).

### Known AI Quirk
AI cars do not have speed clamping. Since `ai_aggressiveness > 1.0` scales throttle to > 1.0, highly aggressive AI can accelerate indefinitely.

---

## Camera System (`src/camera.rs`)

Single `MainCamera` entity spawned at `Startup` (guarded with `if query.is_empty()` to prevent duplicates across state transitions).

**Follow logic** (runs only in `Racing`):
```
target_pos = player.translation + player.back() * 10.0 + Y * 5.0
camera.translation = lerp(camera.translation, target_pos, 0.1)
camera.look_at(player.translation + Y * 2.0)
```

The minimap uses a second camera (`MinimapCamera`, `order: 1`) with a viewport in the bottom-right corner (280×280 px). It sees `RenderLayers::from_layers(&[0, 1])` — the main world plus minimap overlay objects. Track outline segments and vehicle markers are on `RenderLayers::layer(1)` so only the minimap camera renders them.

---

## HUD System (`src/hud.rs`)

All HUD entities are tagged `HudEntity` and despawned on `OnExit(Racing)`.

### Components and What They Drive

| Marker | Updated by | Displays |
|---|---|---|
| `PlaceText` | `update_place_and_hud` | "Place: Nth / 13" |
| `TimeText` | `update_place_and_hud` | Lap number, total time, current lap time |
| `TeslaSpeedText` | `update_tesla_hud` | Speed in MPH (velocity magnitude * 2.23694) |
| `TeslaPowerBar` | `update_tesla_hud` | Right-side energy bar, grows with throttle |
| `TeslaRegenBar` | `update_tesla_hud` | Left-side bar, grows with braking/reverse throttle |
| `TeslaDriftInd` | `update_tesla_hud` | "(P) E-BRAKE" text, orange when drifting |
| `TeslaBrakeInd` | `update_tesla_hud` | "BRAKE" text, red when braking |

### Placement Sorting (`update_place_and_hud`)
All `LapTracker` entities are sorted each frame:
1. By `finished_time` ascending (finished racers ranked first by their time)
2. Then by `current_lap` descending
3. Then by `next_waypoint` descending
4. Then by distance to next waypoint ascending

Player finishes the race when `current_lap > total_laps`, which triggers `GameState::Scoreboard`.

### Minimap Markers
`add_minimap_markers` is an additive system — each frame it checks all `Vehicle` entities for a `MinimapMarker` and spawns one if missing. Player = red sphere, AI = blue sphere, floating 50 units above each vehicle. These are layer-1 objects and only appear in the minimap camera.

---

## UI System (`src/ui.rs`)

### Splash Screen
Full-screen `splash.jpg` image, auto-advances after 3 seconds via `SplashTimer`.

### Loading Screen
Full-screen dark background with "GENERATING RACETRACK..." text and `loading.jpg` preview image (800×450 px). Waits 0.5 s then transitions to `GeneratingLevel`.

### Main Menu
All five difficulty settings use the same pattern: two `Button` entities (Decrease/Increase) with a display `Text` entity in between. The display text is only updated when `GameDifficulty.is_changed()`.

Menu setting ranges:
- AI Aggression: 0.2 – 3.0, step 0.2
- Steering Sens: 0.5 – 10.0, step 0.5
- Top Speed: 40 – 300, step 10
- Acceleration: 50 – 2000, step 50
- Laps: 1 – 10, step 1

### Scoreboard
Spawned on `OnEnter(Scoreboard)`. Shows all 13 racers (player + 12 AI) sorted by finish time or lap progress. Player row is highlighted green. "Main Menu" → `MainMenu`, "Continue" → `PostRace` (currently a dead end).

---

## Lap Tracking (`LapTracker` component)

Both player and AI vehicles have a `LapTracker`. Fields:
- `current_lap`: starts at 1, increments each time `next_waypoint` wraps to 0
- `total_laps`: from `GameDifficulty.laps`
- `next_waypoint`: index into `LevelData.waypoints`, advance threshold = 15 m
- `race_start_time` / `current_lap_start_time`: initialized to `time.elapsed_secs()` on first update frame (detected by checking `== 0.0`)
- `finished_time`: set to `now - race_start_time` when `current_lap > total_laps`
- `place`: stored on the component but **not actually written** — placement is computed dynamically each frame in `update_place_and_hud`

**Note**: the player's `next_waypoint` starts at `1` (skipping waypoint 0 = start/finish). The race does not have a dedicated countdown — vehicles can move immediately.

---

## Entity Cleanup

The `RaceEntity` marker component is attached to everything spawned for a race (terrain, buildings, gates, vehicles, smoke particles, HUD). `cleanup_racing` despawns all of them on `OnExit(Racing)`. The `MainCamera` is NOT a `RaceEntity` — it persists across states.

`HudEntity` is a separate cleanup tag used for HUD-only entities (including the minimap camera).

---

## Known Issues and Gaps

### Critical
- **`PostRace` state is a dead end** — no system exits it. The "Continue" button on the scoreboard routes here and the game freezes in limbo. Fix: add `OnEnter(PostRace)` system that transitions to `GeneratingLevel` or `MainMenu`.
- **`max_speed` is never enforced** — `Vehicle.max_speed` is set but never read in physics. Cars have no speed cap; rely on drag balance.

### Physics / Gameplay
- **AI steering is frame-rate dependent** — the `* 0.1` lerp in `ai_update` is not multiplied by `dt`. At 120 FPS, AI steers twice as fast as at 60 FPS.
- **Smoke particles spawn from all `ExhaustPort` entities every frame** (including AI cars), unconditionally — no throttle check. AI and idle player cars emit smoke constantly.
- **No lap-time split recording for AI** — `LapTracker.lap_times` is never pushed to for either player or AI. The vector exists but is always empty.
- **No collision/bump response** — vehicles can clip through each other at high speed; Rapier handles it via CCD but there's no gameplay response (no knockback, no spin-out).
- **Gate color logic is partially broken** — `update_gate_colors` creates a new material handle every frame for every gate, leaking material assets. It also does not correctly mark passed gates green (the `else if` branch logic has a gap).

### Architecture
- **Duplicate vehicle spawn code** — `vehicle.rs` and `ai.rs` both contain nearly identical wheel/exhaust spawn blocks. This should be extracted to a shared function.
- **`LevelData` not reset between races** — `LevelData` is `Default`-initialized once. If the player races twice, the old waypoints persist until `generate_level` overwrites them. This is fine as long as level gen always runs before vehicles spawn (the state machine guarantees this).
- **No asset loading system** — `splash.jpg` and `loading.jpg` are loaded via `asset_server.load()` inside a system. If the system runs before assets are ready, images may flash blank. Bevy's asset system handles this gracefully with a placeholder, but there's no progress tracking.

---

## Extension Points for Future Iterations

### Easy Wins
- Enforce `max_speed`: in `vehicle_update`, clamp `velocity.linear.length()` or add a counter-force when over the cap
- Fix `PostRace`: add `OnEnter(PostRace)` → transition to `GeneratingLevel` for a "next race" flow
- Fix AI steering to be frame-rate independent: multiply the `0.1` lerp by `dt * target_rate`
- Throttle smoke emission: only spawn smoke when `vehicle.throttle > 0.0`
- Fix gate color material leak: pre-spawn material handles and store them as resources, swap handles instead of creating new ones

### Medium Complexity
- **Sound**: Bevy supports audio natively — engine hum, screech on drift, finish fanfare
- **Track variety**: `get_terrain_height` can be parameterized (seed, frequency, amplitude) to generate distinct terrain shapes per race
- **Vehicle skins**: add a color parameter to `Vehicle` and use it during spawn; let the menu pick colors
- **Countdown timer**: spawn a 3-2-1 UI sequence on `OnEnter(Racing)`, freeze `vehicle_update` for its duration
- **Lap split times**: push to `LapTracker.lap_times` when `next_waypoint` wraps to 0; display on HUD

### Complex
- **Multiplayer / split-screen**: Bevy supports multiple cameras with distinct viewports — a second `Camera3d` with a left/right half-screen viewport, a second `Player` entity driven by gamepad input
- **Car customization**: distinct meshes per car body type, stored as separate assets
- **Track editor**: expose waypoint positions as `ResMut`, allow drag-and-drop in a separate game state
- **Proper suspension**: replace the torque-based self-righting with per-wheel raycasts (Rapier supports `QueryPipeline::cast_ray`) to compute suspension compression and camber
