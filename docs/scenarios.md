# Scénarios concrets (Phase 3)

Three **minimal** automations that match the current **rules + MQTT** surface (no new schema). Each ends with an **observable** outcome: journal + derived state (and optional MQTT command path).

## 1. Mouvement → lumière (dérivé)

**Flow:** capteur publie sur `sensors/motion/{room}` → `dispatch_mqtt_publish` → preset `home` (R1) émet `TurnOnLight` → état dérivé `light_on(room)`.

**Try it**

1. `rusthome serve --data-dir ./data` (ou broker externe + `mqtt_motion_ingest`).
2. `mosquitto_pub -t 'sensors/motion/hall' -m ''` (ou `-m hall`).
3. `rusthome state --data-dir ./data` ou UI `/api/state` — la lumière `hall` est **on**.

**Test de régression:** [`mqtt_observation_closed_loop.rs`](../crates/app/tests/mqtt_observation_closed_loop.rs) (`motion_via_mqtt_turns_on_light_home_preset`).

## 2. Extinction manuelle / scène → commande MQTT

**Flow:** autre système (ou UI web) publie `commands/light/{room}/off` → journal `TurnOffLight` → état éteint.

**Try it**

1. Après le scénario 1, `mosquitto_pub -t 'commands/light/hall/off' -n`.
2. Vérifier `state` / UI — `hall` **off**.

**Test:** [`mqtt_command_ingest.rs`](../crates/app/tests/mqtt_command_ingest.rs).

## 3. Cohérence monde réel — Observed vs dérivé

**Flow:** le moteur dérive l’état depuis règles; un adaptateur séparé peut **observer** l’état réel (ampoule, relais) et écrire des faits **Observed** (`append_observed_light_fact`) lorsque le monde diffère — voir [reconciliation.md](reconciliation.md).

**Try it**

- Exemple CLI / lib : [integration.md — append_observed](integration.md#example-binaries-templates) (`append_observed` example).
- En exploitation: script GPIO ou lecture MQTT « statut réel » qui appelle la même API que l’exemple.

**When to add Observed:** après commande ou si la lumière a été changée hors rusthome (interrupteur physique).

---

Scénarios supplémentaires (température / contact) : [sensor_rules tests](../crates/app/tests/sensor_rules.rs) et [mqtt-contract.md](mqtt-contract.md).
