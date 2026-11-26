# Physical model, assumptions and limitations

This document describes what the simulator actually computes. It is intentionally explicit about approximations so the interface never implies more accuracy than the model supports.

## 1. Surface representation

- The planet is an icosphere (subdivision level 4–7 in the creation UI; 2–8 accepted by the core). Level 6 (default) = 10,242 cells, level 7 = 40,962 cells.
- Every vertex is a simulation cell with an area computed from the spherical triangles that share it. All global means are area-weighted.
- Cells store bedrock elevation (metres relative to a datum), ocean depth, lake depth, ice thickness, snow, air temperature, mixed-layer water temperature, soil water, precipitation, humidity, pressure, cloud cover, albedo, river discharge, flow target, biome, plate, local forcing and volcanic/seismic activity.
- Rendering displaces the mesh radially for readability. This is visual only; stored elevations are real metres.

## 2. Time stepping

- Simulation time is stored in years as `f64`.
- The UI batches real time to roughly 10–14 updates per second, then advances the simulation by `speed × real_dt`.
- Each update is split into at most eight slices. A slice is therefore at most ~0.02 years at ordinary speeds, and grows to millions of years at geological speeds.
- All slow processes use schemes that are stable for arbitrarily large steps:
  - temperature relaxation uses `T + (T_target − T)(1 − e^(−dt/τ))`;
  - aerosol/dust opacity decays exponentially;
  - erosion relaxes exponentially;
  - carbon exchange uses an exponential equilibration fraction;
  - tectonic uplift is clamped per step.
- Seasonal contrast is resolved only when a step is shorter than two years. At faster speeds the annual-mean insolation is used; this is a deliberate resolution choice, not a hidden animation.
- Weather events are subsampled at geological speeds. Short events cannot be resolved individually over millions of years; their aggregate effect is intentionally small.

## 3. Radiative energy balance

A one-layer, latitude-resolved energy balance model is used.

- Annual mean top-of-atmosphere insolation:

  `F(φ) = S/4 · (1 − 0.482 P₂(sin φ))`

  with `P₂(x) = (3x² − 1)/2`. Daily insolation uses the standard hour-angle formula with solar declination from axial tilt and day of year.
- Planetary albedo combines surface albedo and cloud fraction:

  `α_p = α_s (1 − c) + 0.44 c`

  where `c` is cloud coverage.
- Absorbed flux:

  `F_abs = S (1 − α_p) / 4 · exp(−0.65 τ_aerosol)`

  The exponential is a broadband Beer–Lambert approximation for volcanic aerosol and impact dust.
- Greenhouse strength:

  `g = 0.41 + 1.6 · (5.35 ln(C/280) + F_CH₄)/238 + 0.004 (T − 15)`

  clamped to `[0.05, 0.74]`. The 1.6 factor is a fast water-vapour/lapse-rate feedback, not a full radiative transfer calculation.
- Radiative equilibrium:

  `T_eq = (F_abs / (σ(1−g)))^0.25 − 273.15`
- Local temperature target = global equilibrium + latitude bias from annual insolation + seasonal bias + local forcing − elevation lapse (6.5 K/km).
- Land relaxes with τ ≈ 0.12 years (0.35 if icy); ocean mixed layer with τ ≈ 2.4 × heat-capacity years. This produces realistic land–sea contrast and seasonal lag.
- Horizontal heat transport is a diffusive blend toward the neighbour mean, applied only for steps < 10 years (maximum 10% per step). At geological steps the latitude profile already parameterises transport.
- Rotation modifies the heat-transport/weather scaling modestly; it is not a Coriolis solver.

## 4. Carbon

- Atmospheric CO₂ is well mixed. The UI shows the same value everywhere and says so.
- Reservoirs: atmosphere, ocean (2.8× initial atmospheric ppm-equivalent), geological (120×), plus explicit `external_ppm` for user additions.
- Ocean exchange equilibrates toward a temperature-dependent solubility with τ = 280 years, conserving the atmosphere+ocean total.
- Silicate weathering moves carbon from the atmosphere to the geological reservoir with a slow temperature/CO₂-dependent rate.
- Volcanism and impacts outgas carbon from the geological reservoir. If the reservoir is insufficient, the difference is added explicitly.
- Changing CO₂ with “redistribute” moves carbon between reservoirs; changing it without redistribution changes the total inventory and is labelled as such.
- These reservoir ratios are illustrative, not a calibrated carbon-cycle inversion.

## 5. Hydrology and sea level

- Total water = ocean + ice + soil is conserved. Water vapour and lakes are tracked but not included in the eustatic budget (their mass is small and lake geometry is approximate).
- Potential evaporation:
  - open water: `0.30 + 0.03 max(T,0)` m/yr, reduced under ice;
  - land: `(0.015 + 0.0025 max(T,0)) · soil/0.20` m/yr.
- Precipitation is distributed from the evaporated water with weights depending on latitude, orography, humidity and temperature. This guarantees global precipitation = global evaporation at steady state and makes rain patterns follow the simulated relief and climate.
- Soil is a bucket with 0.4 m capacity. Rainfall fills it, evaporation drains it, and saturation excess becomes runoff; 18% of rainfall runs off directly.
- Runoff is routed downhill on the cell graph. Rivers are drawn where discharge exceeds 15 m³/s. Local minima with enough discharge are marked as lakes. Lake storage is diagnostic and does not drive sea level.
- Snow accumulates below 0.5 °C, capped at 3.5 km land / 1.5 m sea ice; excess is returned as runoff/icebergs. Melt returns water to soil or ocean.
- Sea level is solved from the water budget: find `s` such that the ocean volume at `s` equals total water − ice − soil. Up to six Newton-style iterations are used, then the grid depths are updated.
- Thermal expansion is reported as an additional eustatic offset (`2e-4 · ΔT · mean depth`) but is not allowed to create water mass; it shifts the displayed/effective sea level.
- Initial land/water coverage is calibrated by choosing the sea-level quantile of the generated elevation field. The generated water inventory then sets mean ocean depth. The creation screen shows the produced values after generation.

## 6. Ice

- Permanent ice thickness grows from snowfall when `T < 0.5 °C` and melts above 0 °C at `0.12 + 0.008T` m/yr.
- Land ice is capped at 3.5 km; sea ice at 1.5 m. Excess snow becomes runoff.
- Ice and snow raise albedo to 0.62–0.84, creating the ice–albedo feedback.
- Glacial erosion removes a small amount of bedrock proportional to ice thickness and time.
- No ice-flow, calving or basal-sliding solver is implemented.

## 7. Tectonics

- 2–24 plates are spherical caps with a seed centre, an Euler pole/axis and an angular speed of 0.002–0.020 rad/Myr. Their centres rotate over time.
- Cells are assigned to the nearest plate centre; the assignment is recomputed at each tectonic step (nominal cadence 5,000 years).
- At boundaries, relative velocity determines convergence/divergence:
  - continent–continent convergence → uplift;
  - ocean–continent convergence → continental uplift, oceanic trench and volcanism;
  - ocean–ocean convergence → trench and island-arc volcanism;
  - divergence → mid-ocean ridge or continental rift.
- Uplift is clamped to 2,500 m per step and smoothed across one neighbour ring.
- Erosion relaxes land toward sea level with a precipitation-dependent time scale (default 140 Myr at 1.2 m/yr).
- This is kinematic plate tectonics. There is no mantle convection, slab pull, isostasy or viscoelastic deformation.

## 8. Catastrophes

- **Volcano**: intensity 1–10; CO₂ and aerosol totals scale with intensity. 18% of gas and 30% of aerosol are released immediately, the rest over the specified duration. Lava adds local relief and activity colouring.
- **Asteroid**: mass from a 3,000 kg/m³ sphere; kinetic energy includes the sine of the impact angle. Crater diameter uses a standard energy scaling `1.161 (E/(ρg))^0.25`; depth ≈ 0.12 D. Cells within 2.2 crater radii are displaced with a parabolic bowl and rim. Dust optical depth scales as `0.35 D^1.35`; dust and aerosol settle with 2.5-year and 1.6-year time scales. A thermal pulse is added locally.
- **Glaciation forcing** reduces stellar flux by 14% for the specified duration. Whether a snowball develops is decided by the albedo/ice/CO₂ feedbacks, not by a scripted animation.
- **CO₂ injection**, aerosol cooling, ocean-circulation slowdown and magnetic reversal modify the corresponding state.
- Random events are sampled with Poisson rates per million years and capped per step.
- Magnetic field strength is tracked and recovers over 20,000 years; no climate effect of the magnetic field is claimed.

## 9. Weather

- Events have a kind, centre, angular radius, intensity and duration.
- Generation is weighted by local conditions: hurricanes over warm oceans, snow/cold waves in cold regions, heatwaves/droughts/sandstorms in hot dry regions, wildfires in dry vegetated regions.
- Effects are applied to the same soil, ice, temperature-anomaly and atmospheric fields the rest of the model uses: rainfall wets soil, snow adds ice, heatwaves add a decaying temperature anomaly, fires darken albedo and add CO₂.
- Severe events are logged; ordinary rain/snow is not, to avoid flooding the timeline.
- No cloud-resolving model, lightning physics or particle-level precipitation is simulated. The cloud veil in the surface shading is driven by the cloud field.

## 10. What is explicitly not modelled

- Full 3-D atmospheric dynamics, Navier–Stokes, cloud microphysics, atmospheric chemistry.
- Full ocean general circulation, salinity, sea-ice rheology.
- Biogeochemistry, vegetation dynamics, species, soil chemistry.
- Ice-sheet flow, calving, basal hydrology.
- Mantle convection, dynamic topography, true subduction geometry.
- Magnetic dynamo physics and cosmic-ray/climate coupling.
- Tidal, orbital-eccentricity/Milanković forcing beyond obliquity seasons.
- Relativistic or high-energy impact physics beyond the energy scaling above.

The simulator prefers a stable, interconnected, transparent approximation over a visually impressive but disconnected simulation.
