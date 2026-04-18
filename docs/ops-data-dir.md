# Exploitation : `data-dir`, sauvegardes, service (Phase 4)

## Contenu typique de `--data-dir`


| Fichier         | Rôle                                                                           |
| --------------- | ------------------------------------------------------------------------------ |
| `events.jsonl`  | Journal canonique (append-only)                                                |
| `rusthome.toml` | Preset règles, limites d’exécution, etc. (voir [user-rules.md](user-rules.md)) |
| Snapshots       | Produits par `rusthome snapshot` (voir [implementation.md](implementation.md)) |


Gardez **un seul writer** sur le journal (un service `rusthome` ou un adaptateur à la fois).

## Sauvegarde

**Minimum recommandé avant toute mise à jour ou `repair` :**

```bash
DATA=/path/to/data
ts=$(date +%Y%m%d-%H%M%S)
cp -a "$DATA/events.jsonl" "$DATA/events.jsonl.bak-$ts"
# Optional: full directory
tar -czvf "rusthome-data-$ts.tar.gz" -C "$(dirname "$DATA")" "$(basename "$DATA")"
```

Restaurer : arrêter les services, remplacer `events.jsonl`, redémarrer.

## Taille du journal et rotation

Il n’y a pas de rotation automatique du JSONL : planifier **archivage** (copie puis troncature contrôlée) seulement avec une stratégie claire (snapshots + [schema-migration.md](schema-migration.md) si migration). En cas de corruption de **dernière ligne**, voir `rusthome repair` dans [implementation.md](implementation.md) et [errors.md](errors.md).

## systemd

Unité de référence : `[configs/rusthome.service](../configs/rusthome.service)` (tout-en-un : broker + ingest + web).

Points à ajuster par machine :

- `User`, `WorkingDirectory`, chemins `ExecStart` et `--data-dir`
- `ReadWritePaths` doit inclure le répertoire **data** réel
- `--bind` : par défaut loopback dans la doc sécurité ; LAN seulement derrière proxy/TLS — [web-proxy.md](web-proxy.md)

```bash
sudo cp configs/rusthome.service /etc/systemd/system/rusthome.service
sudo systemctl daemon-reload
sudo systemctl enable --now rusthome
journalctl -u rusthome -f
```

Variantes **broker externe** : `[configs/rusthome-mqtt.service](../configs/rusthome-mqtt.service)`, `[configs/rusthome-web.service](../configs/rusthome-web.service)` (voir [integration.md](integration.md)).

## Santé et diagnostic

- HTTP : `GET /api/health` (lab web)
- Traces / `explain` : CLI documentée dans [implementation.md](implementation.md)

## Sécurité réseau (rappel)

L’UI lab **n’a pas d’authentification**. Message d’avertissement côté serveur quand le bind n’est pas loopback : `[crates/web/src/security.rs](../crates/web/src/security.rs)`. Pour l’exposition réseau : proxy inverse + TLS, ou VPN, ou bastion — pas de simple « ouvert sur Internet ».