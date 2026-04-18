# Présence Bluetooth → journal / MQTT (Phase 5, optionnel)

## État produit actuel

Le tableau de bord peut afficher l’inventaire BLE et une **recherche** d’appareil (`bluetoothctl`, etc.) — voir [`crates/web/src/bluetooth_info.rs`](../crates/web/src/bluetooth_info.rs). Ceci est **lecture seule** : aucun événement n’est écrit au journal tant qu’un adaptateur séparé ne le fait pas.

## Quand brancher la présence sur les règles

Intéressant si vous voulez des scénarios du type « téléphone vu → mode maison » **sans** ajouter un nouveau type d’événement persistant : rester sur les mécanismes existants jusqu’à stabilisation cœur + MQTT (phases 1–2 du plan produit domotique — ne pas figer ici).

## Option A — Aucun changement de schéma (recommandé pour prototypes)

Publier vers des topics **déjà** reconnus par [`mqtt_ingest`](mqtt-contract.md), par exemple :

- Traduire « device X visible » en **proxy de mouvement** : `mosquitto_pub -t 'sensors/motion/presence-hall' -m ''` avec une pièce fixe ou JSON `{"room":"hall"}`.
- Ou déclencher un petit script qui appelle `rusthome emit` / API interne si vous préférez le CLI au MQTT.

Limite : la sémantique est « motion-like », pas une vraie entité « présence » dans le core.

## Option B — Nouvel événement persistant

Exigerait :

1. Extension du modèle d’événements + bump `schema_version` — [schema-migration.md](schema-migration.md)
2. Règles dédiées — [rules-changelog.md](rules-changelog.md), [onboarding-rules.md](onboarding-rules.md)

À réserver à une phase où le journal MQTT de base est **stable** (Phases 1–2 du plan domotique).

## Exemple de glue shell (illustratif)

```bash
# Hypothèse : script périodique qui teste la présence d'un MAC (à adapter à votre OS)
MAC="AA:BB:CC:DD:EE:FF"
if bluetoothctl info "$MAC" 2>/dev/null | grep -q "Connected: yes"; then
  mosquitto_pub -h 127.0.0.1 -t 'sensors/motion/presence-proxy' -m '{"room":"living"}'
fi
```

Adaptez broker, topic, et politique de dédoublonnage (éviter un flot d’événements par seconde).
