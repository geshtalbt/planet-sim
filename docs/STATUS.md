# Implementation status

Legend: **Implemented** = used by the simulation and exposed in the UI; **Partial** = a working approximation with documented limits; **Planned** = not present in code. This file is kept honest with the source; do not treat the specification as a claim that every item below is finished.

## Core

| Area | Status | Notes |
|---|---|---|
| Rust + Bevy 0.20 application | Implemented | `cargo run` opens the simulator |
| Modular architecture | Implemented | 15+ modules, simulation core independent of Bevy |
| Seeded procedural generation | Implemented | deterministic 3-D gradient/fBm noise, continent masks, plate relief |
| Planet creation screen | Implemented | presets, custom, randomise, seed, live validation |
| 3-D planet + camera | Implemented | icosphere mesh, orbit/zoom, click selection, visible host-star orbit, axial spin, live axial tilt and thin pole-axis line |
| Time engine | Implemented | pause, 15 presets (minute/s → billion years/s), queued steps, checkpoints |
| Statistics | Implemented | computed from cells/reservoirs, no synthetic values |
| Save/load | Implemented | versioned gzip JSON, topology rebuilt on load |
| History / checkpoints | Partial | adaptive sample history + full-state checkpoints; save file stores samples/events, not old checkpoints |
| What-if forks | Partial | full independent planet forks with synchronised stepping and history comparison; no side-by-side twin 3-D viewport |
| Local intervention | Implemented | temperature offsets, rain, drought, lake water, volcano |
| 15 visual layers + legends | Implemented | all requested layers have data-driven colour scales |
| Rivers, lakes, clouds, atmosphere | Partial | rivers are a line mesh, clouds a data-driven surface veil, atmosphere an unlit shell; no volumetric/particle effects |
| Weather visual effects | Partial | events affect the state and are listed/marked by layer colour; no animated rain/snow/lightning particles |
| GPU compute (wgpu/WGSL) | Planned | CPU/Rayon implementation is currently sufficient for 40k cells; no artificial GPU dependency |

## Physics

| Area | Status | Notes |
|---|---|---|
| One-layer energy balance | Implemented | insolation, cloud/surface albedo, CO₂/CH₄ forcing, water-vapour feedback, aerosols |
| Seasonal cycle | Implemented | daily insolation when step < 2 years, annual mean otherwise |
| Regional temperature field | Implemented | latitude/elevation/ocean heat capacity/diffusion |
| CO₂ live control | Implemented | immediate + ramp, redistribute vs external |
| Axial tilt control | Implemented | immediate + gradual ramp |
| Stellar flux control | Implemented | immediate + gradual ramp + event multipliers |
| Pressure/cloud/albedo/ocean heat capacity | Implemented | direct controls with real effects |
| Total water inventory | Implemented | rebalances ocean and solves sea level |
| Carbon cycle | Partial | 3 reservoirs + exchange/weathering/outgassing; illustrative ratios |
| Evaporation/precipitation | Implemented | mass-conserving normalised distribution |
| Soil/runoff/rivers | Implemented | downhill routing, river discharge, diagnostic lakes |
| Groundwater flow | Planned | soil bucket only |
| Glacier growth/melt | Implemented | mass-balance approximation, 12% land-ice water cap, geothermal melt, CO₂ outgassing recovery |
| Ice-sheet flow / calving | Planned | — |
| Sea level | Implemented | water-budget solver + thermal-expansion offset |
| Tectonic plates | Partial | kinematic Euler-pole motion and boundary uplift/trenches/erosion |
| Mantle convection / isostasy | Planned | — |
| Volcanoes | Implemented | CO₂, aerosols, lava relief, duration |
| Asteroid impacts | Implemented | energy-scaled crater deformation, dust, thermal pulse |
| Ice-age forcing | Implemented | stellar dimming; snowball emerges via feedbacks |
| Ocean circulation change | Partial | heat-transport multiplier + regional response |
| Magnetic field | Partial | tracked reversal/recovery; no dynamo or climate coupling |
| Weather events | Partial | 12 kinds, regional effects; stochastic, not fluid-resolved |
| Biome transitions | Implemented | temperature/precipitation/ice classification each climate step |
| River/lake dynamics | Partial | rivers reform each hydrology step; lakes are diagnostic sinks |
| Coastlines / sea-level change | Implemented | cell depths and ocean area recomputed |
| Deltas / floodplains | Planned | sediment routing not implemented |

## UI / UX

| Screen / feature | Status |
|---|---|
| Main menu | Implemented |
| Planet creation | Implemented |
| Simulation | Implemented |
| Planet statistics | Implemented as a panel |
| Planetary physics | Implemented as a panel |
| Catastrophe management | Implemented as a panel |
| What If? | Implemented as a panel |
| Planet history | Implemented as a panel |
| Saved planets | Implemented as a panel |
| Application settings | Implemented as a panel (subset) |
| Russian UI / README | Implemented — top bar, main menu and settings language switch; `README.ru.md` |
| Graphs | Implemented (temperature, CO₂, sea level, ice, land, precipitation, pressure, albedo, tilt, solar, water, eruptions, magnetic) |
| Parameter initial/current/deviation | Implemented for main physics controls |
| Direct region inspection | Implemented |
| Side-by-side 3-D comparison | Planned |
| Internationalisation | Implemented — English/Русский switch, `LANG` detection, Russian README; free-form historical descriptions may retain the language in which they were recorded |

## Known correctness constraints

- The model is not validated against Earth observations. It reproduces direction and order-of-magnitude behaviour, not quantitative climate prediction.
- At extreme speeds (≥ 1 Myr/s), weather and seasonal detail are intentionally averaged/subsampled.
- The sea-level solver clamps to the bedrock range; extreme water inventories may produce an all-ocean or no-ocean world.
- Lakes do not participate in the eustatic water budget; this can bias local water storage but not global sea level.
- Pole temperatures are damped by the parameterised energy balance; they are not a full polar climate.
- Save files do not include old checkpoints; after load, a checkpoint at the current time is created. History samples/events and full current state are restored.
- Changing bed topography via impacts/tectonics updates depths immediately, but the water budget is re-solved on the next hydrology step.

## Next steps that would materially improve fidelity

1. Replace the diffusive transport with a shallow-water/energy-transport model on the sphere.
2. Couple a proper carbonate–silicate cycle with a calibrated carbon inventory.
3. Implement ice-sheet mass balance with flow and calving.
4. Add a subduction/ridge geometry model and isostatic compensation.
5. Add GPU (WGSL) kernels for per-cell climate/hydrology at level 7–8.
6. Add a second linked 3-D viewport for direct what-if visual comparison.
