# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Run the game (dev build — deps are pre-optimized via Cargo.toml)
cargo run

# Optimized release build
cargo build --release

# Check for compile errors without building
cargo check
```

There are no tests in this project.

## Architecture

**Rust Racer** is a 3D racing game built with [Bevy 0.18.1](https://bevyengine.org/) and Bevy Rapier3D for physics.

### Game State Flow

States are defined in `src/game_state.rs`:

```
Splash → MainMenu → Loading → GeneratingLevel → Racing → Scoreboard → (PostRace) → MainMenu
```

`GameDifficulty` is a Bevy resource set in the menu and read throughout — it carries AI aggressiveness, steering sensitivity, top speed, acceleration, and lap count.

### Source Modules

| File | Responsibility |
|---|---|
| `main.rs` | App entry point, plugin registration, lighting |
| `game_state.rs` | `GameState` enum, `GameDifficulty` resource, `LapTracker` component |
| `vehicle.rs` | Physics-based vehicle: player input, throttle/brake/drift, lateral grip, self-righting torque, exhaust particles |
| `ai.rs` | 12 AI opponents with waypoint navigation, stuck detection, reversing, and three skill tiers |
| `level_gen.rs` | Procedural 401×401 heightfield terrain, circular waypoint layout (24–40 points), distance-based road coloring |
| `camera.rs` | Lerp-based follow camera behind the player |
| `hud.rs` | In-race HUD: lap/time/placement, Tesla-style speedometer, minimap, scoreboard |
| `ui.rs` | Splash screen, loading screen, main menu with difficulty sliders |

### Physics & Vehicle Model

- Chassis collider: 2.0×1.0×4.0 rounded cuboid via Rapier
- Lateral grip: `30.0` normally, `8.0` while drifting (Space or Shift)
- Steering max angle: `1.047 rad` (60°); sensitivity is a `GameDifficulty` field
- Self-righting uses terrain normal + torque — **spring stiffness is intentionally low** to prevent physics explosions on slopes
- `get_terrain_height(x, z)` in `level_gen.rs` computes terrain elevation at any world position using the same sine/cosine formula as mesh generation; keep these in sync if you change terrain math

### AI System

AI opponents are spawned in a staggered 4×3 grid at race start. Three skill tiers relative to player specs: +10% (4 cars), 1.0× (4 cars), 0.9× (4 cars). Each `AiDrivatar` component stores its current waypoint index, stuck timer, and reversing state.

### Dependency Optimization

`Cargo.toml` sets `opt-level = 3` for all dependencies in dev profile. This is intentional for playable frame rates during development — do not remove it.
