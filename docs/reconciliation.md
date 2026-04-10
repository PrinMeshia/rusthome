# Journal ↔ real-world reconciliation (plan §14.7)

## Truth invariant (V0+ / EPIC 1)

**Observed facts are the source of truth for modelled physical state.** Derived facts are a projection: they must **converge** toward observed state.

If the projection (via a last **Derived** light fact) **contradicts** an incoming observation (**Observed** `LightOn` / `LightOff`) for the same entity (`entity_id` = room for V0 lights):

1. **Observation wins**: the observed fact is applied after correction.
2. The system **appends** an audit fact `StateCorrectedFromObservation { entity_id, expected, observed }` (**Derived** provenance, emitted by the pipeline) first, then the **Observed** light fact.

Reference test: **Derived `LightOn` → Observed `LightOff` → final state OFF** and journal explicitly contains the correction line; **replay** reproduces the same state.

**Who emits the observation?** The core engine does not sense the physical world. Something **outside** the pure rule pipeline must append **Observed** facts: e.g. a device driver (GPIO readback, smart-bulb API), an integration service, or a human operator using CLI `observed-light`. That layer decides *when* and *how* reality is sampled; rusthome only reconciles once an Observed line is offered.

**Limits of V0:** `LightOn` / `LightOff` are coarse. They do not distinguish “lamp commanded off”, “burnt bulb”, “no power”, or “sensor fault”. Modeling a dead bulb or actuator fault would need **new event types** (or richer payloads) and a clear rule for which component maps hardware signals into journal lines—still integration responsibility, not the FIFO engine.

API entry: `rusthome_app::append_observed_light_fact` / CLI `observed-light --timestamp … --room … --state on|off`.

## Simulation (`physical_projection_mode = Simulation`)

- The journal can assert a light **ON** via a **Derived** fact with no field bus.
- **Expected**: gap between projection and real bulb; normal for lab / demo.
- **Product**: surface simulation mode explicitly (plan §14.5).

## IoAnchored

- “Physical” facts (e.g. light state) must be **Observed** or come from the **§6.16** cycle (modelled IO success/failure).
- V0 **rejects** rules emitting **Derived** `LightOn` / `LightOff` (see `RunError::IoAnchoredDerivedActuator`).
- **If device diverges from journal**: append **Observed** facts (or use `observed-light`); the engine logs `StateCorrectedFromObservation` when projection was **Derived**.

## Before 24/7 prod (plan §14.6)

- **Dead letter**: store rejects outside canonical journal + alert.
- Deterministic **failure fact** to unblock a cascade.
- **Quarantine**: read-only mode + operator decision (repair §8.5, replay).

See also [errors.md](errors.md).
